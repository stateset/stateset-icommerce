# Developer-led commerce evaluation

A platform engineer evaluating subscription commerce can bring one workflow and compare it with Next Commerce, Whop, an internal build or another finalist. The StateSet evaluation gives the team a storefront they control and a goal they can test: recover a failed membership renewal, trace the original charge to the native payment and billing cycle, and account for a partial refund. Use sandbox data for the evaluation. A team with an agent-operated store can also demonstrate a governed write and inspect its receipt. [Storefront guide](../packages/create-stateset-app/README.md) · [Agent setup](../AGENTS.md)

## Join or close a pilot

Email the maintainers at **hello@stateset.io** with the subject **Merchant platform pilot**. Include the workflow, the alternatives on the shortlist, how you found StateSet, and a target decision date. A maintainer opens one private evaluation row per independent team when the evaluation begins. At the target date, ask which platform the team selected and which factor decided it. A team can report a competitor win, a decision to buy nothing, or an ongoing evaluation through the same address. Keep customer names, payment references, and credentials out of this exchange. [Maintainer contact](../CONTRIBUTING.md#questions)

For a StateSet selection, invite the merchant to enroll a production store voluntarily before its first sale. Use `npm run merchant -- adoption-enroll --source CHANNEL --apply`. After a real primary card order, the operator can run `npm run --silent merchant -- adoption-report --check` to obtain the aggregate report and installation ID. In a private review, the maintainer confirms the report's source and that the merchant recognizes the matching production transaction; only the aggregate report is retained. Teams can participate in the evaluation and decline this follow-up.

## Run the private scorecard

Keep the ledger and submitted reports outside the repository. Start the ledger as an empty JSON array. Add one object for every enrolled team, including losses and teams that stop responding. The scorecard reads only the local files named by the operator.

| Ledger field | Operator entry |
| --- | --- |
| `id`, `enrolledOn`, `source` | Stable anonymous team label, UTC enrollment date, and acquisition channel such as `developer-referral`. Use lowercase letters, numbers, hyphens, or underscores for labels. |
| `alternatives` | Array of distinct shortlist labels, such as `next-commerce`, `whop`, or `internal-build`. An empty array records an undisclosed shortlist. |
| `decision` | Omit until the buyer decides. Then supply `on` (UTC date), `selected` (a listed alternative, `stateset`, or `no-purchase`), and `reason` (a short buyer-reported reason label). Use `undisclosed` when no reason was provided. |
| `installationId` | Optional UUID from the enrolled storefront. One installation can support one team in the ledger. |
| `reviews` | Optional array of private review entries. Each includes `reviewedOn` (UTC date) and `report` (path to the original submitted JSON, relative to the ledger). Append reviews so historical reports can be reproduced. |

```bash
node scripts/pilot-scorecard.mjs --ledger /private/pilots/evaluations.json \
  --from 2026-11-01 --through 2026-11-30 --as-of 2026-12-31
```

The inclusive enrollment window selects the cohort. `--as-of` sets the last date on which decisions and report reviews count, and defaults to the end of enrollment. **Decided win rate** divides explicit StateSet selections by all resolved decisions, including decisions to buy nothing. **Activated winning teams per enrolled** divides evidence-backed wins by every team that entered the evaluation. A production activation requires a StateSet selection and that installation's latest reviewed report with complete coverage and a verified live primary card sale since pilot enrollment. Sandbox sales and package downloads contribute zero activations. An empty denominator produces a null percentage.

Review `evidenceAmongWins` to distinguish missing reports, incomplete provider coverage, and completed checks with no qualifying sale, `nonWinReasons` to identify the next product obstacle, and the source breakdown to see which channels yield operating stores. Each shortlist breakdown covers all teams that considered that alternative, including teams that chose a third option. A team may appear in multiple shortlist breakdowns. The self-selected pilot supports comparisons within this cohort. Estimating global market share requires an independent count of the whole target market. One verified card sale also leaves the outcomes model's repeat-use and retention milestones open. [Outcomes model](./OUTCOMES_MODEL.md#stage-b-market-proof) · [Competitive landscape](./COMPETITIVE_LANDSCAPE.md)
