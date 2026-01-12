# Multi-Currency Commerce with Stablecoins

Cross-border commerce using stablecoins as the universal settlement layer.

## Overview

Stablecoins simplify international commerce:
- No foreign exchange fees
- No currency conversion delays
- Same-day settlement worldwide
- Single currency accounting

## Price Display

### Multi-Currency Pricing

```bash
# Set up store currencies
stateset --apply "enable currencies USD, EUR, GBP, JPY, CAD"
stateset --apply "set exchange rate USD to EUR at 0.92"
stateset --apply "set exchange rate USD to GBP at 0.79"
stateset --apply "set exchange rate USD to JPY at 149.50"
stateset --apply "set exchange rate USD to CAD at 1.36"

# Products priced in USD, displayed in local currency
stateset "show product WIDGET-001 in EUR"

# Output:
# Widget Pro
#   USD: $99.99
#   EUR: €91.99
```

### Customer's Local Currency

```bash
# German customer views cart
stateset --resume sess_xyz "show cart in EUR"

# Output:
# Cart (EUR)
#   ─────────────────────────────────────────
#   2x Widget Pro           €183.98
#   1x Accessory Kit        €27.59
#   ─────────────────────────────────────────
#   Subtotal:               €211.57
#   Shipping (DE):          €12.99
#   VAT (19%):              €42.67
#   ─────────────────────────────────────────
#   Total:                  €267.23
#
#   Pay in USDC:            $290.47 USDC
```

## Cross-Border Checkout

### European Customer

```bash
# Customer in Germany orders from US merchant

# 1. Create cart
stateset --apply "create cart for hans@example.de"

# 2. Add items (prices shown in EUR)
stateset --apply --resume sess_de "add 1x Widget Pro"
stateset --apply --resume sess_de "show cart in EUR"

# 3. Set shipping to Germany
stateset --apply --resume sess_de "ship to Hauptstraße 123, 10115 Berlin, Germany"

# 4. Calculate with EU VAT
stateset --resume sess_de "calculate total with VAT"

# Output:
# Order Total (Germany)
#   Product:     €91.99
#   Shipping:    €15.99
#   VAT (19%):   €20.52
#   ─────────────
#   Total EUR:   €128.50
#   Pay USDC:    $139.67

# 5. Customer pays in USDC (universal stablecoin)
stateset pay --apply \
  --to 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM \
  --amount 139.67 \
  --chain solana

# 6. Complete checkout
stateset --apply --resume sess_de "complete checkout"
```

### Japanese Customer

```bash
# Customer in Japan

# 1. Create cart
stateset --apply "create cart for tanaka@example.jp"

# 2. View prices in JPY
stateset --apply --resume sess_jp "add 1x Widget Pro"
stateset --resume sess_jp "show cart in JPY"

# Output:
# Cart (JPY)
#   1x Widget Pro    ¥14,949
#   Shipping (JP):   ¥2,500
#   ─────────────────
#   Total:           ¥17,449
#   Pay in USDC:     $116.72

# 3. Customer pays in USDC
stateset pay --apply \
  --to 9WzD... \
  --amount 116.72 \
  --chain solana
```

## Merchant Settlement

### All Payments in USDC

```bash
# Regardless of customer location, merchant receives USDC
stateset pay --balance --chain solana

# Output:
# Balance: 15,234.67 USDC
#
# Recent deposits:
#   $139.67  Germany customer    2 mins ago
#   $116.72  Japan customer      15 mins ago
#   $89.99   UK customer         1 hour ago
#   $199.99  Canada customer     2 hours ago
```

### Convert to Fiat (Optional)

```bash
# If merchant wants to convert to local fiat
# Option 1: Coinbase offramp
stateset pay --apply \
  --to 0xCoinbaseWallet... \
  --amount 5000.00 \
  --chain base \
  --memo "Weekly settlement to USD"

# Option 2: Keep as USDC for payments
# No conversion needed for digital-native business

# Option 3: Convert to ssUSD for yield
stateset pay --apply \
  --to 0xYourSetChainWallet... \
  --amount 5000.00 \
  --chain set_chain \
  --memo "Move to ssUSD for yield"
```

## Supplier Payments (International)

### Pay Chinese Supplier

```bash
# Traditional: $25-50 wire fee, 3-5 days, FX spread
# Stablecoin: $0.01 fee, instant, no FX

# 1. Receive invoice from Chinese supplier
#    Amount: ¥50,000 CNY (~$7,000 USD)

# 2. Pay in USDC (supplier accepts)
stateset pay --apply \
  --to 0xChineseSupplierWallet... \
  --amount 7000.00 \
  --chain ethereum \
  --order PO-2024-CN-001 \
  --memo "Electronics components - Invoice INV-CN-5678"

# Supplier receives USDC, converts to CNY locally
# Total fees: ~$10 (vs $50+ wire fee + 2-3% FX spread)
```

### Pay European Vendor

```bash
# Pay EU contractor in USDC
stateset pay --apply \
  --to 0xEuropeanVendorWallet... \
  --amount 5000.00 \
  --chain arbitrum \
  --memo "January consulting services"

# Vendor can:
# - Keep as USDC
# - Convert to EUR via local exchange
# - Use for own supplier payments
```

## Multi-Region Store

### Regional Pricing Strategy

```bash
# Set regional prices
stateset --apply "set product WIDGET-001 prices: \
  USD: 99.99 \
  EUR: 94.99 \
  GBP: 84.99 \
  JPY: 14500"

# Customer sees local price, pays in USDC
stateset "show WIDGET-001 for customer in UK"

# Output:
# Widget Pro
#   Price: £84.99
#   Pay: $107.58 USDC
```

### Tax Handling by Region

```bash
# US customer (state tax)
stateset "calculate tax for order to California"
# → 8.25% sales tax

# EU customer (VAT)
stateset "calculate tax for order to Germany"
# → 19% VAT

# UK customer (VAT)
stateset "calculate tax for order to UK"
# → 20% VAT

# Canada customer (GST/HST)
stateset "calculate tax for order to Ontario, Canada"
# → 13% HST
```

## Currency Reporting

### Revenue by Region

```bash
stateset "show revenue by region for Q4 2023"

# Output:
# Revenue by Region - Q4 2023
#   ─────────────────────────────────────────
#   United States      $125,430.00   (52%)
#   Europe (EUR)       $48,720.00    (20%)
#   United Kingdom     $31,200.00    (13%)
#   Canada             $19,500.00    (8%)
#   Japan              $12,150.00    (5%)
#   Other              $4,800.00     (2%)
#   ─────────────────────────────────────────
#   Total (USDC):      $241,800.00
#
#   All settlements in USDC - no FX exposure
```

### Exchange Rate Impact

```bash
stateset "analyze exchange rate impact for January 2024"

# Output:
# Exchange Rate Analysis - January 2024
#   ─────────────────────────────────────────
#   EUR/USD movement: -1.2%
#   GBP/USD movement: +0.8%
#   JPY/USD movement: -2.5%
#
#   Impact on displayed prices:
#     EUR customers see +1.2% higher USD prices
#     GBP customers see -0.8% lower USD prices
#     JPY customers see +2.5% higher USD prices
#
#   Revenue impact: $0 (all settled in USDC)
#
#   Note: Consider updating regional prices if
#   exchange rates move >5% for extended period
#   ─────────────────────────────────────────
```

## Stablecoin as Universal Currency

### Benefits

```bash
# Why USDC as settlement layer:

# 1. Universal acceptance
#    - Works globally without banking relationships
#    - No correspondent bank fees

# 2. Instant settlement
#    - No 3-5 day wire delays
#    - 24/7/365 availability

# 3. No FX risk for merchant
#    - Price in USD, receive USD
#    - Customer handles local conversion

# 4. Low fees
#    - $0.01-0.05 per transaction
#    - vs 2-4% for FX + credit card

# 5. Transparent pricing
#    - Customer sees exact USD amount
#    - No hidden FX spreads
```

### Customer Experience

```bash
# Customer flow:

# 1. Browse in local currency (comfort)
#    "Show me products in EUR"

# 2. See final USD amount at checkout
#    "Total: €128.50 (Pay: 139.67 USDC)"

# 3. Pay from any USDC wallet
#    - No bank account needed
#    - No credit card required
#    - Works from any country

# 4. Receive instant confirmation
#    - Order processed immediately
#    - No payment hold/verification
```

## Multi-Chain Settlement

### Accept from Any Chain

```bash
# Customer can pay from their preferred chain

# Display all payment options
stateset "show payment options for cart CART-12345"

# Output:
# Payment Options
#   Amount: $139.67 USDC
#
#   Chain          Address                              Est. Fee
#   ─────────────────────────────────────────────────────────────
#   Solana         9WzD...AWWM                          $0.005
#   Base           0x742d...fE21                        $0.01
#   Arbitrum       0x742d...fE21                        $0.008
#   SET Chain      0x742d...fE21 (ssUSD)                $0.001
#   Ethereum       0x742d...fE21                        $10-30
#
# Recommendation: Solana or Arbitrum for lowest fees
```

## Summary

| Traditional International | Stablecoin Commerce |
|--------------------------|---------------------|
| Wire fees: $25-50 | Tx fee: $0.01-0.05 |
| Settlement: 3-5 days | Settlement: < 1 min |
| FX spread: 2-4% | FX: 0% (USDC to USDC) |
| Banking hours only | 24/7/365 |
| Bank account required | Any crypto wallet |
| Country restrictions | Global access |
