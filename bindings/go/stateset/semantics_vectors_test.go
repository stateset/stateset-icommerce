// The Go binding against the shared semantic corpus.
//
// `bindings/test-vectors/semantics-v1.json` pins the MEANING of money across
// every binding, and `crates/stateset-embedded/tests/semantics_vectors.rs`
// keeps that file honest against the engine. This file asserts the categories
// the Go surface can reach and DECLARES the rest with the reason; the
// declaration is checked against the corpus, so a new category cannot slip in
// unnoticed.
//
// Go note: `Orders.Create` takes `currency string`, and Go has no "absent"
// string, so the zero value "" means "not given" here. That is why the
// corpus's blank-currency row is skipped below rather than asserted.
package stateset

import (
	"encoding/json"
	"math/big"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

type semanticsCorpus struct {
	Version    int                        `json:"version"`
	Categories map[string]json.RawMessage `json:"categories"`
}

type semanticsCategory struct {
	Rows []map[string]any `json:"rows"`
}

var semanticsNotReachable = map[string]string{
	"currency_decimals":  "the Go surface exposes no currency-scale accessor",
	"canadian_tax_rates": "the Go surface exposes no Canadian tax lookup",
}

var semanticsAsserted = []string{
	"decimal_render",
	"money_scale_enforced",
	"rejected_inputs",
	"accepted_inputs",
}

func loadSemantics(t *testing.T) semanticsCorpus {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join("..", "..", "test-vectors", "semantics-v1.json"))
	if err != nil {
		t.Fatalf("read corpus: %v", err)
	}
	var doc semanticsCorpus
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatalf("parse corpus: %v", err)
	}
	if doc.Version != 1 {
		t.Fatalf("corpus version %d, want 1", doc.Version)
	}
	return doc
}

func semanticsRows(t *testing.T, doc semanticsCorpus, name string) []map[string]any {
	t.Helper()
	var cat semanticsCategory
	if err := json.Unmarshal(doc.Categories[name], &cat); err != nil {
		t.Fatalf("category %s: %v", name, err)
	}
	return cat.Rows
}

func newVectorsStore(t *testing.T) (*Commerce, *Customer) {
	t.Helper()
	c, err := New(":memory:")
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	t.Cleanup(c.Close)
	cust, err := c.Customers().Create("vectors@example.com", "V", "E", "")
	if err != nil {
		t.Fatalf("customer: %v", err)
	}
	return c, cust
}

func line(cust *Customer, price string, qty int32) OrderItem {
	return OrderItem{ProductID: cust.ID, SKU: "SKU-1", Name: "Widget", Quantity: qty, UnitPrice: price}
}

// Exact decimal equality, independent of how many trailing zeros each side
// carries, without routing either value through a float.
func sameDecimal(a, b string) bool {
	x, okx := new(big.Rat).SetString(a)
	y, oky := new(big.Rat).SetString(b)
	return okx && oky && x.Cmp(y) == 0
}

func TestSemanticsEveryCategoryAccountedFor(t *testing.T) {
	doc := loadSemantics(t)
	var present, accounted []string
	for name := range doc.Categories {
		present = append(present, name)
	}
	accounted = append(accounted, semanticsAsserted...)
	for name := range semanticsNotReachable {
		accounted = append(accounted, name)
	}
	sort.Strings(present)
	sort.Strings(accounted)
	if len(present) != len(accounted) {
		t.Fatalf("corpus has %v, binding accounts for %v", present, accounted)
	}
	for i := range present {
		if present[i] != accounted[i] {
			t.Fatalf("corpus has %v, binding accounts for %v -- assert it or declare why not", present, accounted)
		}
	}
}

func TestSemanticsDecimalRender(t *testing.T) {
	doc := loadSemantics(t)
	c, cust := newVectorsStore(t)
	for _, row := range semanticsRows(t, doc, "decimal_render") {
		if ok, _ := row["money_scale_ok"].(bool); !ok {
			continue
		}
		ops := row["operands"].([]any)
		var items []OrderItem
		switch row["op"] {
		case "add":
			for _, o := range ops {
				items = append(items, line(cust, o.(string), 1))
			}
		case "mul":
			qty := new(big.Rat)
			qty.SetString(ops[1].(string))
			items = []OrderItem{line(cust, ops[0].(string), int32(qty.Num().Int64()))}
		default:
			continue
		}
		order, err := c.Orders().Create(cust.ID, items, "")
		if err != nil {
			t.Fatalf("%v: create: %v", row["id"], err)
		}
		if !sameDecimal(order.TotalAmount, row["expected"].(string)) {
			t.Errorf("%v: total %s, corpus says %s", row["id"], order.TotalAmount, row["expected"])
		}
	}
}

func TestSemanticsMoneyScaleEnforced(t *testing.T) {
	doc := loadSemantics(t)
	c, cust := newVectorsStore(t)
	for _, row := range semanticsRows(t, doc, "money_scale_enforced") {
		if row["currency"] != "USD" {
			continue // orders here use the store default currency
		}
		_, err := c.Orders().Create(cust.ID, []OrderItem{line(cust, row["amount"].(string), 1)}, "")
		if mustReject, _ := row["must_reject"].(bool); mustReject && err == nil {
			t.Errorf("%v: %v must be refused", row["id"], row["amount"])
		} else if !mustReject && err != nil {
			t.Errorf("%v: %v must be accepted: %v", row["id"], row["amount"], err)
		}
	}
}

func TestSemanticsRejectedInputs(t *testing.T) {
	doc := loadSemantics(t)
	c, cust := newVectorsStore(t)
	for _, row := range semanticsRows(t, doc, "rejected_inputs") {
		value := row["value"].(string)
		var err error
		switch row["kind"] {
		case "currency":
			if value == "" {
				continue // Go's zero value: "not given", see the file comment
			}
			_, err = c.Orders().Create(cust.ID, []OrderItem{line(cust, "10.00", 1)}, value)
		case "uuid":
			_, err = c.Orders().Create(value, []OrderItem{line(cust, "10.00", 1)}, "")
		default:
			continue // timestamps and dates have no order-path field
		}
		if err == nil {
			t.Errorf("%v: %q must be refused", row["id"], value)
		}
	}
	orders, err := c.Orders().List()
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(orders) != 0 {
		t.Errorf("refused inputs wrote %d orders", len(orders))
	}
}

func TestSemanticsAcceptedInputs(t *testing.T) {
	doc := loadSemantics(t)
	c, cust := newVectorsStore(t)
	for _, row := range semanticsRows(t, doc, "accepted_inputs") {
		if row["kind"] != "currency" {
			continue
		}
		order, err := c.Orders().Create(cust.ID, []OrderItem{line(cust, "10.00", 1)}, row["value"].(string))
		if err != nil {
			t.Fatalf("%v: %v must stay acceptable: %v", row["id"], row["value"], err)
		}
		if order.Currency != row["normalizes_to"] {
			t.Errorf("%v: currency %s, want %v", row["id"], order.Currency, row["normalizes_to"])
		}
	}
}
