package stateset

import "testing"

// Go has no "absent" string, and every wrapper passes its arguments through
// C.CString, so "" is the only way a Go caller can mean "not given". The
// binding used to send it on as a real empty value, which the engine rightly
// rejects -- so a customer could not be created without a phone.
func TestOptionalStringsCanBeOmitted(t *testing.T) {
	c, err := New(":memory:")
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	defer c.Close()

	cust, err := c.Customers().Create("nophone@example.com", "No", "Phone", "")
	if err != nil {
		t.Fatalf("a customer without a phone must be creatable: %v", err)
	}
	if cust.Phone != nil && *cust.Phone != "" {
		t.Errorf("phone should be absent, got %q", *cust.Phone)
	}

	if _, err := c.Customers().Create("withphone@example.com", "With", "Phone", "+1-555-0123"); err != nil {
		t.Fatalf("a supplied phone still works: %v", err)
	}
}
