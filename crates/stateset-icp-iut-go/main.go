// ICP-1.0 conformance IUT — Go reference.
//
// Reads one JSON object from stdin, dispatches on the test name passed in
// argv[1], writes one JSON object to stdout. Protocol: see
// icp-conformance/iut-adapters/iut.protocol.md.
//
// Pure Go stdlib. No external dependencies.
package main

import (
	"bytes"
	"crypto/ecdh"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"os"
	"sort"
	"strconv"
	"strings"
	"unicode/utf16"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "FATAL: missing test name argument")
		os.Exit(2)
	}
	testName := os.Args[1]

	rawInput, err := io.ReadAll(os.Stdin)
	if err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: read stdin: %v\n", err)
		os.Exit(2)
	}
	// Use json.Decoder with UseNumber to preserve original number string forms.
	dec := json.NewDecoder(bytes.NewReader(rawInput))
	dec.UseNumber()
	var input map[string]interface{}
	if err := dec.Decode(&input); err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: parse stdin JSON: %v\n", err)
		os.Exit(2)
	}

	var output map[string]interface{}
	switch testName {
	case "01-aid-derivation":
		output, err = run01AidDerivation(input)
	case "02-canonical-json":
		output, err = run02CanonicalJson(input)
	case "03-signature-verification":
		output, err = run03SignatureVerification(input)
	default:
		// Per iut.protocol.md: exit 2 + JSON on stderr signals SKIP.
		fmt.Fprintf(os.Stderr, `{"error":"unsupported","reason":"no handler for %s"}`+"\n", testName)
		os.Exit(2)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: %v\n", err)
		os.Exit(1)
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(output); err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: encode output: %v\n", err)
		os.Exit(1)
	}
}

// ---------------------------------------------------------------------------
// Test 01: AID derivation and Intent signing
// ---------------------------------------------------------------------------

func run01AidDerivation(input map[string]interface{}) (map[string]interface{}, error) {
	agent, ok := input["agent"].(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("missing 'agent' in input")
	}
	edSeedHex, ok := agent["ed25519_seed_hex"].(string)
	if !ok {
		return nil, fmt.Errorf("missing agent.ed25519_seed_hex")
	}
	xSeedHex, ok := agent["x25519_seed_hex"].(string)
	if !ok {
		return nil, fmt.Errorf("missing agent.x25519_seed_hex")
	}

	edSeed, err := hex.DecodeString(edSeedHex)
	if err != nil || len(edSeed) != 32 {
		return nil, fmt.Errorf("ed25519_seed must be 32 hex bytes")
	}
	xSeed, err := hex.DecodeString(xSeedHex)
	if err != nil || len(xSeed) != 32 {
		return nil, fmt.Errorf("x25519_seed must be 32 hex bytes")
	}

	// --- Keypairs ----------------------------------------------------------
	edPriv := ed25519.NewKeyFromSeed(edSeed)
	edPub := edPriv.Public().(ed25519.PublicKey)
	if len(edPub) != 32 {
		return nil, fmt.Errorf("unexpected Ed25519 pubkey length %d", len(edPub))
	}

	xCurve := ecdh.X25519()
	xPriv, err := xCurve.NewPrivateKey(xSeed)
	if err != nil {
		return nil, fmt.Errorf("x25519 NewPrivateKey: %w", err)
	}
	xPubBytes := xPriv.PublicKey().Bytes()

	// --- AID per ICP-1.0 §4.2 ---------------------------------------------
	hasher := sha256.New()
	hasher.Write(edPub)
	hasher.Write([]byte{0x00})
	hasher.Write(xPubBytes)
	digest := hasher.Sum(nil)
	aid := "aid:v1:z" + base58btcEncode(digest)

	// --- Build Intent: fill buyer + principal_binding.agent ----------------
	intentVal, ok := input["intent"].(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("missing 'intent' in input")
	}
	// Deep-copy via re-marshal so we don't mutate the input map.
	intentBytes, _ := json.Marshal(intentVal)
	dec := json.NewDecoder(bytes.NewReader(intentBytes))
	dec.UseNumber()
	var intent map[string]interface{}
	if err := dec.Decode(&intent); err != nil {
		return nil, fmt.Errorf("clone intent: %w", err)
	}
	intent["buyer"] = aid
	if pb, ok := intent["principal_binding"].(map[string]interface{}); ok {
		pb["agent"] = aid
	}

	// --- Canonicalize and sign --------------------------------------------
	canonical, err := canonicalJSON(intent)
	if err != nil {
		return nil, fmt.Errorf("canonicalize: %w", err)
	}
	sig := ed25519.Sign(edPriv, []byte(canonical))

	out := map[string]interface{}{
		"ed25519_pubkey_hex":         hex.EncodeToString(edPub),
		"x25519_pubkey_hex":          hex.EncodeToString(xPubBytes),
		"aid":                        aid,
		"intent_canonical_string":    canonical,
		"intent_canonical_bytes_hex": hex.EncodeToString([]byte(canonical)),
		"intent_signature_hex":       hex.EncodeToString(sig),
	}

	// --- Optional tamper-rejected check ------------------------------------
	if params, ok := input["params"].(map[string]interface{}); ok {
		if verify, ok := params["verify_tamper_rejected"].(bool); ok && verify {
			tampered := strings.Replace(canonical, "29.99", "0.01", 1)
			ok := ed25519.Verify(edPub, []byte(tampered), sig)
			out["tamper_rejected"] = !ok
		}
	}

	return out, nil
}

// ---------------------------------------------------------------------------
// Test 02: Canonical JSON
// ---------------------------------------------------------------------------

func run02CanonicalJson(input map[string]interface{}) (map[string]interface{}, error) {
	casesRaw, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	canonicalStrings := make([]string, 0, len(casesRaw))
	names := make([]string, 0, len(casesRaw))
	for i, c := range casesRaw {
		caseMap, ok := c.(map[string]interface{})
		if !ok {
			return nil, fmt.Errorf("case %d not an object", i)
		}
		name, _ := caseMap["name"].(string)
		canonical, err := canonicalJSON(caseMap["value"])
		if err != nil {
			return nil, fmt.Errorf("case %s: %w", name, err)
		}
		canonicalStrings = append(canonicalStrings, canonical)
		names = append(names, name)
	}
	return map[string]interface{}{
		"canonical_strings": canonicalStrings,
		"names":             names,
	}, nil
}

// ---------------------------------------------------------------------------
// Test 03: Signature Verification
// ---------------------------------------------------------------------------

func run03SignatureVerification(input map[string]interface{}) (map[string]interface{}, error) {
	casesRaw, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	verifications := make([]bool, 0, len(casesRaw))
	names := make([]string, 0, len(casesRaw))
	for i, c := range casesRaw {
		caseMap, ok := c.(map[string]interface{})
		if !ok {
			return nil, fmt.Errorf("case %d not an object", i)
		}
		canonical, _ := caseMap["canonical"].(string)
		signatureHex, _ := caseMap["signature_hex"].(string)
		pubkeyHex, _ := caseMap["pubkey_hex"].(string)
		name, _ := caseMap["name"].(string)
		verifications = append(verifications, verifyOne(canonical, signatureHex, pubkeyHex))
		names = append(names, name)
	}
	return map[string]interface{}{
		"verifications": verifications,
		"names":         names,
	}, nil
}

func verifyOne(canonical, signatureHex, pubkeyHex string) bool {
	sigBytes, err := hex.DecodeString(signatureHex)
	if err != nil || len(sigBytes) != 64 {
		return false
	}
	pubBytes, err := hex.DecodeString(pubkeyHex)
	if err != nil || len(pubBytes) != 32 {
		return false
	}
	return ed25519.Verify(ed25519.PublicKey(pubBytes), []byte(canonical), sigBytes)
}

// ---------------------------------------------------------------------------
// Canonical JSON encoder
//
// Matches the JS IUT's canonicalJson and the Rust IUT's serde_jcs output per
// RFC 8785 on ICP-1.0 payload shapes (objects, arrays, strings,
// integers/decimals, booleans, null). Produced by lexicographic key ordering
// + no whitespace + standard JSON escapes (no HTML-safety escaping, per
// RFC 8785 §3.2.2.2) + ECMAScript shortest-form number serialization
// (RFC 8785 §3.2.2.3).
// ---------------------------------------------------------------------------

func canonicalJSON(v interface{}) (string, error) {
	var buf bytes.Buffer
	if err := writeCanonical(&buf, v); err != nil {
		return "", err
	}
	return buf.String(), nil
}

func writeCanonical(w *bytes.Buffer, v interface{}) error {
	switch x := v.(type) {
	case nil:
		w.WriteString("null")
	case bool:
		if x {
			w.WriteString("true")
		} else {
			w.WriteString("false")
		}
	case string:
		w.WriteString(encodeCanonicalString(x))
	case json.Number:
		// RFC 8785 treats every JSON number as an IEEE-754 double: parse the
		// literal and re-serialize in ECMAScript shortest form ("1.50" → "1.5").
		f, err := strconv.ParseFloat(string(x), 64)
		if err != nil {
			return fmt.Errorf("number %q is not an IEEE-754 double: %w", string(x), err)
		}
		s, err := formatCanonicalNumber(f)
		if err != nil {
			return err
		}
		w.WriteString(s)
	case []interface{}:
		w.WriteByte('[')
		for i, item := range x {
			if i > 0 {
				w.WriteByte(',')
			}
			if err := writeCanonical(w, item); err != nil {
				return err
			}
		}
		w.WriteByte(']')
	case map[string]interface{}:
		keys := make([]string, 0, len(x))
		for k := range x {
			keys = append(keys, k)
		}
		// RFC 8785 §3.2.3: sort by UTF-16 code unit (what JS Array.prototype
		// .sort does). This differs from sort.Strings (UTF-8 byte = code
		// point order) when an astral-plane key — whose UTF-16 form starts
		// with a surrogate in 0xD800–0xDBFF — is compared against a BMP key
		// above U+DFFF. (serde_jcs 0.1.0 sorts by UTF-8 bytes instead; no
		// ICP-1.0 payload shape carries astral-plane keys, so the Rust IUT
		// stays byte-identical on the conformance vectors.)
		sort.Slice(keys, func(i, j int) bool { return lessUTF16(keys[i], keys[j]) })
		w.WriteByte('{')
		for i, k := range keys {
			if i > 0 {
				w.WriteByte(',')
			}
			w.WriteString(encodeCanonicalString(k))
			w.WriteByte(':')
			if err := writeCanonical(w, x[k]); err != nil {
				return err
			}
		}
		w.WriteByte('}')
	case float64:
		// json.Decoder without UseNumber returns numbers as float64. The
		// canonical IUT path should only hit this when input is parsed
		// without UseNumber.
		s, err := formatCanonicalNumber(x)
		if err != nil {
			return err
		}
		w.WriteString(s)
	default:
		return fmt.Errorf("canonicalJSON: unsupported type %T", v)
	}
	return nil
}

// encodeCanonicalString encodes s as a JSON string per RFC 8785 §3.2.2.2,
// byte-identical to ECMAScript JSON.stringify: two-character escapes for
// `"` `\` \b \f \n \r \t, lowercase \u00xx for the remaining control
// characters below U+0020, and raw UTF-8 for everything else — including
// `<`, `>`, `&` (no HTML-safety escaping), U+007F, and U+2028/U+2029 (no
// JSONP-safety escaping). encoding/json diverges on all three classes
// (HTML escapes, \u0008/\u000c instead of \b/\f, and escaped
// line/paragraph separators), so the escaper is hand-rolled.
func encodeCanonicalString(s string) string {
	var out strings.Builder
	out.Grow(len(s) + 2)
	out.WriteByte('"')
	for _, r := range s {
		switch r {
		case '"':
			out.WriteString(`\"`)
		case '\\':
			out.WriteString(`\\`)
		case '\b':
			out.WriteString(`\b`)
		case '\f':
			out.WriteString(`\f`)
		case '\n':
			out.WriteString(`\n`)
		case '\r':
			out.WriteString(`\r`)
		case '\t':
			out.WriteString(`\t`)
		default:
			if r < 0x20 {
				fmt.Fprintf(&out, `\u%04x`, r)
			} else {
				out.WriteRune(r)
			}
		}
	}
	out.WriteByte('"')
	return out.String()
}

// lessUTF16 reports whether a sorts before b in lexicographic UTF-16
// code-unit order (RFC 8785 §3.2.3 / ECMAScript string comparison).
func lessUTF16(a, b string) bool {
	ua := utf16.Encode([]rune(a))
	ub := utf16.Encode([]rune(b))
	for i := 0; i < len(ua) && i < len(ub); i++ {
		if ua[i] != ub[i] {
			return ua[i] < ub[i]
		}
	}
	return len(ua) < len(ub)
}

// formatCanonicalNumber serializes an IEEE-754 double per RFC 8785 §3.2.2.3,
// i.e. ECMAScript Number::toString semantics (the same bytes JSON.stringify
// produces): shortest round-trip digits, plain decimal notation for
// |x| in [1e-6, 1e21), exponent notation with explicit sign and no
// leading-zero exponent digits otherwise, and "0" for negative zero.
func formatCanonicalNumber(f float64) (string, error) {
	if math.IsNaN(f) || math.IsInf(f, 0) {
		return "", fmt.Errorf("non-finite number %v cannot be canonicalized", f)
	}
	if f == 0 {
		// Covers -0: ECMAScript Number::toString(-0) is "0".
		return "0", nil
	}
	if abs := math.Abs(f); abs >= 1e-6 && abs < 1e21 {
		return strconv.FormatFloat(f, 'f', -1, 64), nil
	}
	// Exponent range. Go pads the exponent to two digits ("1e-07"); ECMAScript
	// does not ("1e-7"). Both keep the explicit sign.
	s := strconv.FormatFloat(f, 'e', -1, 64)
	mantissa, exp, _ := strings.Cut(s, "e")
	digits := strings.TrimLeft(exp[1:], "0")
	if digits == "" {
		digits = "0"
	}
	return mantissa + "e" + exp[:1] + digits, nil
}

// ---------------------------------------------------------------------------
// Base58btc (Bitcoin alphabet)
//
// Identical algorithm to JS and Rust IUTs: arbitrary-precision base
// conversion via byte-array long division, with leading-zero preservation
// rendered as '1' chars.
// ---------------------------------------------------------------------------

func base58btcEncode(buf []byte) string {
	const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

	// Count leading zero bytes.
	leading := 0
	for _, b := range buf {
		if b == 0 {
			leading++
		} else {
			break
		}
	}

	// Working copy for in-place division.
	input := make([]byte, len(buf))
	copy(input, buf)

	digits := make([]byte, 0, len(buf)*2)
	start := leading
	for start < len(input) {
		var carry uint32
		for i := start; i < len(input); i++ {
			v := uint32(input[i]) + carry*256
			input[i] = byte(v / 58)
			carry = v % 58
		}
		digits = append(digits, byte(carry))
		for start < len(input) && input[start] == 0 {
			start++
		}
	}

	out := make([]byte, 0, leading+len(digits))
	for i := 0; i < leading; i++ {
		out = append(out, '1')
	}
	for i := len(digits) - 1; i >= 0; i-- {
		out = append(out, alphabet[digits[i]])
	}
	return string(out)
}
