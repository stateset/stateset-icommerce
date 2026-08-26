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
	"math/big"
	"os"
	"regexp"
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
	case "04-escrow-lifecycle":
		output, err = run04EscrowLifecycle(input)
	case "05-intent-validation":
		output, err = run05IntentValidation(input)
	case "06-quote-binding":
		output, err = run06QuoteBinding(input)
	case "07-settlement-receipts":
		output, err = run07SettlementReceipts(input)
	case "08-timing":
		output, err = run08Timing(input)
	case "09-ceilings":
		output, err = run09Ceilings(input)
	case "10-commerce-invariants":
		output, err = run10CommerceInvariants(input)
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

// ---------------------------------------------------------------------------
// 04-escrow-lifecycle — ICP-1.0 §8 state machine + event replay
// ---------------------------------------------------------------------------

// escrowTransitions encodes the normative §8 transition table directly.
var escrowTransitions = map[string]string{
	"pending|payment_confirmed":                   "funded",
	"funded|fulfillment_confirmed_window_elapsed": "released",
	"funded|dispute_raised":                       "disputed",
	"disputed|resolution_favors_merchant":         "released",
	"disputed|resolution_favors_buyer":            "refunded",
	"funded|merchant_cancel_or_expiry":            "refunded",
}

func escrowStep(state, trigger string) map[string]interface{} {
	if next, ok := escrowTransitions[state+"|"+trigger]; ok {
		return map[string]interface{}{"state": next}
	}
	if state == "funded" && trigger == "payment_confirmed" {
		return map[string]interface{}{"error": "escrow.already_funded"}
	}
	return map[string]interface{}{"error": "escrow.wrong_state"}
}

func escrowReplay(events []interface{}) map[string]interface{} {
	state := "pending"
	for index, raw := range events {
		event, ok := raw.(map[string]interface{})
		if !ok {
			return map[string]interface{}{"error": "escrow.seq_out_of_order"}
		}
		seqNumber, ok := event["seq"].(json.Number)
		if !ok {
			return map[string]interface{}{"error": "escrow.seq_out_of_order"}
		}
		seq, err := seqNumber.Int64()
		if err != nil || int(seq) != index {
			return map[string]interface{}{"error": "escrow.seq_out_of_order"}
		}
		trigger, _ := event["trigger"].(string)
		step := escrowStep(state, trigger)
		if errCode, failed := step["error"]; failed {
			return map[string]interface{}{"error": errCode}
		}
		state = step["state"].(string)
	}
	return map[string]interface{}{"final_state": state}
}

func run04EscrowLifecycle(input map[string]interface{}) (map[string]interface{}, error) {
	transitionCases, ok := input["transition_cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.transition_cases must be an array")
	}
	replayCases, ok := input["replay_cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.replay_cases must be an array")
	}

	transitions := map[string]interface{}{}
	for _, raw := range transitionCases {
		c := raw.(map[string]interface{})
		transitions[c["id"].(string)] = escrowStep(c["from"].(string), c["trigger"].(string))
	}
	replays := map[string]interface{}{}
	for _, raw := range replayCases {
		c := raw.(map[string]interface{})
		events, _ := c["events"].([]interface{})
		replays[c["id"].(string)] = escrowReplay(events)
	}
	return map[string]interface{}{"transitions": transitions, "replays": replays}, nil
}

// ---------------------------------------------------------------------------
// 05-intent-validation — ICP-1.0 §6 intent envelope validation
// ---------------------------------------------------------------------------

var (
	aidRe     = regexp.MustCompile(`^aid:v1:z[1-9A-HJ-NP-Za-km-z]{40,60}$`)
	settlerRe = regexp.MustCompile(`^settler:[a-z0-9]+(\.[a-z0-9]+)*$`)
	moneyRe   = regexp.MustCompile(`^-?[0-9]+(\.[0-9]{1,18})?$`)
)

type intentVerbSpec struct {
	aids          []string
	money         []string
	itemsRequired bool
	required      []string
}

var intentVerbs = map[string]intentVerbSpec{
	"purchase.create": {[]string{"buyer", "merchant"}, []string{"max_total"}, true,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "items", "max_total", "expiry", "principal_binding", "nonce", "iat", "exp"}},
	"inventory.query": {[]string{"buyer", "merchant"}, nil, false,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "principal_binding", "nonce", "iat", "exp"}},
	"quote.request": {[]string{"buyer", "merchant"}, nil, true,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "items", "principal_binding", "nonce", "iat", "exp"}},
	"payout.request": {[]string{"seller", "platform"}, []string{"amount"}, false,
		[]string{"v", "verb", "intent_id", "seller", "platform", "settler", "amount", "destination", "principal_binding", "nonce", "iat", "exp"}},
	"subscription.create": {[]string{"buyer", "merchant"}, []string{"max_total_per_period"}, false,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "service_id", "cadence", "max_total_per_period", "first_charge_at", "principal_binding", "nonce", "iat", "exp"}},
	"subscription.cancel": {[]string{"buyer", "merchant"}, nil, false,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "subscription_id", "effective", "principal_binding", "nonce", "iat", "exp"}},
	"purchase.return": {[]string{"buyer", "merchant"}, nil, true,
		[]string{"v", "verb", "intent_id", "buyer", "merchant", "settler", "original_settlement_id", "items", "desired_outcome", "principal_binding", "nonce", "iat", "exp"}},
}

func validateIntent(raw interface{}) map[string]interface{} {
	intent, ok := raw.(map[string]interface{})
	if !ok {
		return map[string]interface{}{"error": "format.bad_schema"}
	}
	if _, ok := intent["v"]; !ok {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	if v, _ := intent["v"].(string); v != "icp-1.0" {
		return map[string]interface{}{"error": "version.unsupported"}
	}
	if _, ok := intent["verb"]; !ok {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	verb, _ := intent["verb"].(string)
	spec, ok := intentVerbs[verb]
	if !ok {
		return map[string]interface{}{"error": "format.unknown_verb"}
	}
	for _, field := range spec.required {
		if _, ok := intent[field]; !ok {
			return map[string]interface{}{"error": "format.missing_field"}
		}
	}
	for _, field := range spec.aids {
		if !aidRe.MatchString(fmt.Sprintf("%v", intent[field])) {
			return map[string]interface{}{"error": "format.bad_aid"}
		}
	}
	if !settlerRe.MatchString(fmt.Sprintf("%v", intent["settler"])) {
		return map[string]interface{}{"error": "format.bad_settler_id"}
	}
	for _, field := range spec.money {
		m, ok := intent[field].(map[string]interface{})
		if !ok || !moneyRe.MatchString(fmt.Sprintf("%v", m["amount"])) {
			return map[string]interface{}{"error": "format.bad_money"}
		}
	}
	if spec.itemsRequired {
		items, ok := intent["items"].([]interface{})
		if !ok || len(items) < 1 {
			return map[string]interface{}{"error": "format.bad_schema"}
		}
	}
	return map[string]interface{}{"valid": true}
}

func run05IntentValidation(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	validations := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		validations[c["id"].(string)] = validateIntent(c["intent"])
	}
	return map[string]interface{}{"validations": validations}, nil
}

// ---------------------------------------------------------------------------
// 06-quote-binding — ICP-1.0 §11.4 max_total ceiling (exact decimal compare)
// ---------------------------------------------------------------------------

// cmpAmount compares two non-negative decimal strings. Returns -1, 0, or 1.
// Exact — no float conversion.
func cmpAmount(a, b string) int {
	ia, fa := splitDecimal(a)
	ib, fb := splitDecimal(b)
	ia = strings.TrimLeft(ia, "0")
	if ia == "" {
		ia = "0"
	}
	ib = strings.TrimLeft(ib, "0")
	if ib == "" {
		ib = "0"
	}
	if len(ia) != len(ib) {
		if len(ia) < len(ib) {
			return -1
		}
		return 1
	}
	if ia != ib {
		if ia < ib {
			return -1
		}
		return 1
	}
	n := len(fa)
	if len(fb) > n {
		n = len(fb)
	}
	fa = fa + strings.Repeat("0", n-len(fa))
	fb = fb + strings.Repeat("0", n-len(fb))
	if fa == fb {
		return 0
	}
	if fa < fb {
		return -1
	}
	return 1
}

func splitDecimal(s string) (string, string) {
	if i := strings.IndexByte(s, '.'); i >= 0 {
		return s[:i], s[i+1:]
	}
	return s, ""
}

func run06QuoteBinding(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	decisions := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		quote := c["quote_total"].(map[string]interface{})["amount"].(string)
		max := c["intent_max_total"].(map[string]interface{})["amount"].(string)
		if cmpAmount(quote, max) > 0 {
			decisions[c["id"].(string)] = map[string]interface{}{"error": "policy.quote.exceeds_max_total"}
		} else {
			decisions[c["id"].(string)] = map[string]interface{}{"valid": true}
		}
	}
	return map[string]interface{}{"decisions": decisions}, nil
}

// ---------------------------------------------------------------------------
// 07-settlement-receipts — ICP-1.0 §9 co-signed receipt verification
// ---------------------------------------------------------------------------

func sigValue(sig interface{}) (string, bool) {
	m, ok := sig.(map[string]interface{})
	if !ok {
		return "", false
	}
	s, ok := m["sig"].(string)
	return s, ok && s != ""
}

func verifyReceipt(raw interface{}, merchantPk, settlerPk string) map[string]interface{} {
	receipt, ok := raw.(map[string]interface{})
	if !ok {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	merchantSig, ok := sigValue(receipt["merchant_signature"])
	if !ok {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	settlerSig, ok := sigValue(receipt["settler_signature"])
	if !ok {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	unsigned := map[string]interface{}{}
	for k, v := range receipt {
		if k != "merchant_signature" && k != "settler_signature" {
			unsigned[k] = v
		}
	}
	canonical, err := canonicalJSON(unsigned)
	if err != nil {
		return map[string]interface{}{"error": "format.missing_field"}
	}
	if !verifyOne(canonical, merchantSig, merchantPk) {
		return map[string]interface{}{"error": "signature.invalid"}
	}
	if !verifyOne(canonical, settlerSig, settlerPk) {
		return map[string]interface{}{"error": "settlement.settler_signature_invalid"}
	}
	return map[string]interface{}{"valid": true}
}

func run07SettlementReceipts(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	verifications := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		merchantPk, _ := c["merchant_pubkey_hex"].(string)
		settlerPk, _ := c["settler_pubkey_hex"].(string)
		verifications[c["id"].(string)] = verifyReceipt(c["receipt"], merchantPk, settlerPk)
	}
	return map[string]interface{}{"verifications": verifications}, nil
}

// ---------------------------------------------------------------------------
// 08-timing — ICP-1.0 §5.3 replay window (strict parse + shared epoch algo)
// ---------------------------------------------------------------------------

const timingWindowMax = 600 // §5.3 intent window ceiling, seconds

var timingTsRe = regexp.MustCompile(`^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$`)

func daysFromCivil(y, m, d int64) int64 {
	y2 := y
	if m <= 2 {
		y2 = y - 1
	}
	base := y2
	if y2 < 0 {
		base = y2 - 399
	}
	era := base / 400
	yoe := y2 - era*400
	mm := m + 9
	if m > 2 {
		mm = m - 3
	}
	doy := (153*mm+2)/5 + d - 1
	doe := yoe*365 + yoe/4 - yoe/100 + doy
	return era*146097 + doe - 719468
}

// parseEpoch returns (epochSeconds, true) or (0, false) if not a strict
// YYYY-MM-DDTHH:MM:SSZ timestamp with in-range fields.
func parseEpoch(s string) (int64, bool) {
	m := timingTsRe.FindStringSubmatch(s)
	if m == nil {
		return 0, false
	}
	var v [6]int64
	for i := 0; i < 6; i++ {
		n, err := strconv.ParseInt(m[i+1], 10, 64)
		if err != nil {
			return 0, false
		}
		v[i] = n
	}
	y, mo, d, h, mi, se := v[0], v[1], v[2], v[3], v[4], v[5]
	if !(mo >= 1 && mo <= 12 && d >= 1 && d <= 31 && h <= 23 && mi <= 59 && se <= 59) {
		return 0, false
	}
	return daysFromCivil(y, mo, d)*86400 + h*3600 + mi*60 + se, true
}

func validateTiming(iat, exp, now string) map[string]interface{} {
	ti, iok := parseEpoch(iat)
	te, eok := parseEpoch(exp)
	tn, nok := parseEpoch(now)
	if !iok || !eok || !nok {
		return map[string]interface{}{"error": "replay.timestamp_malformed"}
	}
	if te-ti > timingWindowMax {
		return map[string]interface{}{"error": "replay.window_too_long"}
	}
	if te < tn {
		return map[string]interface{}{"error": "replay.expired"}
	}
	return map[string]interface{}{"valid": true}
}

func run08Timing(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	validations := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		iat, _ := c["iat"].(string)
		exp, _ := c["exp"].(string)
		now, _ := c["now"].(string)
		validations[c["id"].(string)] = validateTiming(iat, exp, now)
	}
	return map[string]interface{}{"validations": validations}, nil
}

// ---------------------------------------------------------------------------
// 09-ceilings — refund/payout authoritative ceilings (reuses cmpAmount)
// ---------------------------------------------------------------------------

var ceilingCode = map[string]string{
	"return": "policy.return.exceeds_max_refund",
	"payout": "policy.payout.exceeds_max_per_payout",
}

func run09Ceilings(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	decisions := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		value := c["value"].(map[string]interface{})["amount"].(string)
		ceiling := c["ceiling"].(map[string]interface{})["amount"].(string)
		if cmpAmount(value, ceiling) > 0 {
			decisions[c["id"].(string)] = map[string]interface{}{"error": ceilingCode[c["kind"].(string)]}
		} else {
			decisions[c["id"].(string)] = map[string]interface{}{"valid": true}
		}
	}
	return map[string]interface{}{"decisions": decisions}, nil
}

// ---------------------------------------------------------------------------
// 10-commerce-invariants — economic invariants (exact decimal, reuses cmpAmount)
// ---------------------------------------------------------------------------

// currencyScale maps a currency code to its permitted number of minor units.
var currencyScale = map[string]int{
	"USD":  2,
	"EUR":  2,
	"GBP":  2,
	"JPY":  0,
	"USDC": 6,
	"USDT": 6,
	"ETH":  18,
	"BTC":  8,
}

// amountString coerces a JSON value (string or number) to its decimal text.
func amountString(v interface{}) string {
	switch x := v.(type) {
	case string:
		return x
	case json.Number:
		return x.String()
	case nil:
		return "0"
	}
	return fmt.Sprintf("%v", v)
}

// addAmount sums two non-negative decimal strings exactly, via integer
// arithmetic on a common scale. No float conversion.
func addAmount(a, b string) string {
	ia, fa := splitDecimal(a)
	ib, fb := splitDecimal(b)
	n := len(fa)
	if len(fb) > n {
		n = len(fb)
	}
	xa, ok1 := new(big.Int).SetString(ia+fa+strings.Repeat("0", n-len(fa)), 10)
	xb, ok2 := new(big.Int).SetString(ib+fb+strings.Repeat("0", n-len(fb)), 10)
	if !ok1 || !ok2 {
		return "0"
	}
	return unscale(new(big.Int).Add(xa, xb), n)
}

// subAmount subtracts b from a exactly. The result may be negative.
func subAmount(a, b string) string {
	ia, fa := splitDecimal(a)
	ib, fb := splitDecimal(b)
	n := len(fa)
	if len(fb) > n {
		n = len(fb)
	}
	xa, ok1 := new(big.Int).SetString(ia+fa+strings.Repeat("0", n-len(fa)), 10)
	xb, ok2 := new(big.Int).SetString(ib+fb+strings.Repeat("0", n-len(fb)), 10)
	if !ok1 || !ok2 {
		return "0"
	}
	return unscale(new(big.Int).Sub(xa, xb), n)
}

// unscale renders a scaled big.Int back as a decimal string with n fraction digits.
func unscale(v *big.Int, n int) string {
	neg := v.Sign() < 0
	digits := new(big.Int).Abs(v).String()
	if n > 0 {
		if len(digits) <= n {
			digits = strings.Repeat("0", n-len(digits)+1) + digits
		}
		digits = digits[:len(digits)-n] + "." + digits[len(digits)-n:]
	}
	if neg {
		return "-" + digits
	}
	return digits
}

// isNegative reports whether an exact decimal string is below zero.
func isNegative(s string) bool { return strings.HasPrefix(s, "-") }

// decimalScale counts the significant fraction digits of a decimal string,
// ignoring insignificant trailing zeros (100.000 has scale 0).
func decimalScale(s string) int {
	_, frac := splitDecimal(s)
	return len(strings.TrimRight(frac, "0"))
}

func decideCommerceCase(c map[string]interface{}) map[string]interface{} {
	invalid := func(code string) map[string]interface{} {
		return map[string]interface{}{"error": code}
	}
	valid := map[string]interface{}{"valid": true}

	switch c["kind"].(string) {
	case "refund":
		// Σ refunds (completed + in-flight + requested) MUST NOT exceed captured.
		total := addAmount(addAmount(amountString(c["completed_refunds"]), amountString(c["inflight_refunds"])), amountString(c["requested"]))
		if cmpAmount(total, amountString(c["captured"])) > 0 {
			return invalid("commerce.refund.exceeds_captured")
		}
		return valid

	case "capture":
		// Σ captures (completed + in-flight + requested) MUST NOT exceed the order total.
		total := addAmount(addAmount(amountString(c["completed_captures"]), amountString(c["inflight_captures"])), amountString(c["requested"]))
		if cmpAmount(total, amountString(c["order_total"])) > 0 {
			return invalid("commerce.capture.exceeds_order_total")
		}
		return valid

	case "return_quantity":
		// Nothing may be returned before anything shipped.
		shipped := amountString(c["shipped"])
		if cmpAmount(shipped, "0") <= 0 {
			return invalid("commerce.return.order_not_shipped")
		}
		total := addAmount(amountString(c["already_returned"]), amountString(c["requested"]))
		if cmpAmount(total, shipped) > 0 {
			return invalid("commerce.return.exceeds_shipped")
		}
		return valid

	case "reserve":
		// A reservation never exceeds on_hand − allocated.
		available := subAmount(amountString(c["on_hand"]), amountString(c["allocated"]))
		requested := amountString(c["requested"])
		if isNegative(available) || cmpAmount(requested, available) > 0 {
			return invalid("commerce.inventory.insufficient_available")
		}
		return valid

	case "journal_entry":
		lines, _ := c["lines"].([]interface{})
		debits, credits := "0", "0"
		for _, rawLine := range lines {
			line := rawLine.(map[string]interface{})
			debit := amountString(line["debit"])
			credit := amountString(line["credit"])
			// Every line is single-sided: at most one of debit/credit is non-zero.
			if cmpAmount(debit, "0") > 0 && cmpAmount(credit, "0") > 0 {
				return invalid("commerce.ledger.line_not_single_sided")
			}
			debits = addAmount(debits, debit)
			credits = addAmount(credits, credit)
		}
		if cmpAmount(debits, credits) != 0 {
			return invalid("commerce.ledger.entry_unbalanced")
		}
		return valid

	case "money_scale":
		currency := c["currency"].(string)
		scale, known := currencyScale[strings.ToUpper(currency)]
		if !known {
			scale = 2
		}
		if decimalScale(amountString(c["amount"])) > scale {
			return invalid("commerce.money.scale_exceeds_currency")
		}
		return valid
	}
	return valid
}

func run10CommerceInvariants(input map[string]interface{}) (map[string]interface{}, error) {
	cases, ok := input["cases"].([]interface{})
	if !ok {
		return nil, fmt.Errorf("input.cases must be an array")
	}
	decisions := map[string]interface{}{}
	for _, raw := range cases {
		c := raw.(map[string]interface{})
		decisions[c["id"].(string)] = decideCommerceCase(c)
	}
	return map[string]interface{}{"decisions": decisions}, nil
}
