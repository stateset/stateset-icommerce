# Evaluate StateSet iCommerce

This project contains a working storefront and the StateSet 1.40.1 SDK for Linux x64 with glibc. You can publish a product, try a catalog import and inspect the SQLite store on your machine. Package installation and image builds require internet access. Local product setup and the included checkout simulations run without a payment account.

## Run the shop

From the directory containing the archive and its companion `SHA256SUMS`, verify the download with `sha256sum -c SHA256SUMS` (or `shasum -a 256 -c SHA256SUMS`), then extract the archive and open the `stateset-merchant-pilot` directory.

With Node 20.20+ and npm 10+ on Linux x64 with glibc, run these commands:

```sh
npm ci --include=optional
npm run setup
npm run setup -- --apply
npm run dev -- --hostname 127.0.0.1
```

Or use Docker Compose with a Linux AMD64 container runtime. This path uses its own persistent database volume and binds the storefront to your computer's loopback address. If port 3000 is in use, run `export PILOT_PORT=3100` in the same terminal first.

```sh
docker compose --profile tools build
docker compose run --rm operator npm run setup -- --apply
docker compose up -d web
```

Open the private localhost link printed by setup. Under **Store setup**, create your first product or choose **Import a product catalog** to preview a Next Commerce or Whop export. Open the public storefront in another tab and add an active, in-stock product to the cart. The merchant link expires after one day. Running setup again gives you a new link. For Docker, `docker compose down` stops the shop and retains its data.

## Compare one workflow

| Workflow | What to check |
| --- | --- |
| Next Commerce catalog | Upload `scripts/catalog-examples/next-commerce.json` with source store code `workshop`. Preview the blue mug at USD 24.91 with four units, apply, then select that variant in the shop. Preview the same file again to see which data is preserved. |
| Whop catalog | Upload `scripts/catalog-examples/whop.json`. The USD 12.10 handbook qualifies and the monthly membership is excluded. Apply and confirm the handbook stays a private, zero-stock draft. |
| Embed StateSet beside an existing checkout | Run `npm run verify:embedded-refund`, or `docker compose run --rm operator npm run verify:embedded-refund`. The engine refuses a USD 250 refund against a USD 100 fixture payment. An approved USD 40 refund keeps the same receipt and audit hash when retried after the process exits. |
| Checkout and refunds | Run `npm run verify:storefront` locally, or `docker compose run --rm operator npm run verify:storefront`. The exercises create disposable stores and use simulated payments. |

Both sample files contain fictional catalog data. You can substitute your own saved exports using the same portal steps.

The default checkout uses Base mainnet USDC and starts with a placeholder receiver address. Store setup, catalog review and the local drills work without a wallet. Configure a provider and run the launch checks before taking payments. [Merchant guide](MERCHANT-GUIDE.md)

The app dependencies are pinned in `package-lock.json`. The two StateSet npm archives include the SDK and its Linux binary. Their original checksums, source workspace revision, local-build status and immutable download location are recorded separately from this storefront's source commit. [SDK build record](vendor/stateset/SDK-VERIFICATION.json) · [Native dependency notices](vendor/stateset/notices/native/README.md) · [Storefront revision](STOREFRONT-REVISION) · [StateSet license](LICENSE)

To record a competitive pilot decision, email **hello@stateset.io** with the subject **Merchant platform pilot**. Include the workflow, which platforms you evaluated, and what influenced your decision. Keep private setup links, catalog files and customer or transaction details on your machine.
