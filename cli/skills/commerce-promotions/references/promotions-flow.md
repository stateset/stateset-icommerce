# Promotions and Coupons

## Types

- percentage_off
- fixed_amount_off
- free_shipping
- buy_x_get_y

## Common Commands

- `stateset --apply "create promotion SUMMER20 'Summer Sale' 20% off"`
- `stateset --apply "create coupon SUMMER20 for promotion PROMO-123"`
- `stateset "validate coupon SUMMER20"`
- `stateset --apply "apply coupon SUMMER20 to cart cart_123"`
