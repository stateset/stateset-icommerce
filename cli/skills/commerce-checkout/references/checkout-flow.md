# Checkout Flow (ACP)

## Steps

1. Create cart
2. Add items
3. Set shipping address
4. Select shipping rate
5. Apply discounts
6. Set payment method
7. Complete checkout

## Common Commands

- `stateset --apply "create cart for customer@example.com"`
- `stateset --apply "add 2x WIDGET-001 to cart cart_123"`
- `stateset --apply "set shipping address on cart cart_123 to '123 Main St'"`
- `stateset --apply "apply coupon WELCOME10 to cart cart_123"`
- `stateset --apply "checkout cart cart_123"`
