# subscription-finder — competitor scan (2026-07-10)

Scan of the "find recurring charges / subscription finder" tool space, run before
implementing. All findings paraphrased from public tool pages — no competitor
copy, branding, or trademarks reproduced.

## Competitors skimmed (top real tools)

1. **My Bank Statement Analysis — recurring charges** (mybankstatementanalysis.com/recurring-charges)
   — upload a statement (PDF), lists every recurring charge; privacy-forward (in-memory
   processing, auto-delete). No bank linking.
2. **Substract** (substract.co) — upload statement, "every recurring charge in 60 seconds",
   no bank linking, no account required. Groups by merchant, shows cadence + cost.
3. **Just Cancel** (justcancel.io) — upload a bank statement, AI finds recurring charges in
   ~30s, one-time fee, no bank connection. Emphasises "subscriptions you forgot about".
4. **Nexafin** (nexafin.com) — upload CSV **or** PDF, identifies recurring charges without
   handing over bank credentials.
5. **Bank-connected trackers** (Rocket Money, Quicken Simplifi, Kudos, MoneyPilot) — sync to
   the account, send renewal/trial reminders, offer cancellation help.

Sources: mybankstatementanalysis.com, substract.co, justcancel.io, nexafin.com,
cnbc.com/select/best-subscription-trackers, moneypilot.com.

## Table-stakes features (tagged in-model / out-of-model)

| Feature | Tag | Decision |
| --- | --- | --- |
| Group same merchant + similar amount into one recurring charge | in-model | **built** — normalize the description, match amount within tolerance |
| Detect cadence (weekly / biweekly / monthly / quarterly / annual) | in-model | **built** — median inter-charge interval → cadence class |
| Project annual cost per charge | in-model | **built** — amount × periods/year |
| Total monthly + annual spend | in-model | **built** — header + footer totals |
| Occurrence count / last-seen date per charge | in-model | **built** — `×N`, shown per line |
| Estimate next charge date | in-model | **built** — last date + median interval |
| Paste / CSV input, no bank linking (privacy) | in-model | **built** — paste `date, description, amount` lines; runs fully in-browser |
| Currency formatting | in-model | **built** — `currency` symbol param |
| Sort by cost (highest first) | in-model | **built** — descending annual cost |
| Multiple date formats (ISO / US / EU) | in-model | **built** — `date_format` enum (auto/iso/us/eu) |
| Minimum-occurrences threshold to flag recurring | in-model | **built** — `min_occurrences` param |
| Upload/parse a PDF bank statement | out-of-model | **not built** — needs a PDF-statement parser + layout heuristics; paste CSV/text instead |
| Bank account linking / auto-sync | out-of-model | **not built** — needs a backend + bank aggregator (Plaid etc.); gizza is browser-local, no account |
| Renewal / free-trial reminders | out-of-model | **not built** — needs a server + notifications |
| One-click cancellation help | out-of-model | **not built** — needs merchant integrations |
| Merchant category tagging (streaming/software/…) | out-of-model (mostly) | **not built** — needs a merchant→category DB; keyword guessing is unreliable, declined |

## Descriptor decisions

Params: `transactions` (required, multiline paste), `min_occurrences` (default 2, 2–24),
`currency` (default `$`), `date_format` (enum auto/iso/us/eu, default auto). Amount-match
tolerance kept as an internal constant (5% or $0.50, whichever larger) — no competitor
exposes it and it would bloat the schema. Output: a ranked plain-text report with a
header total, one line per detected charge (name, amount, cadence, count, next-charge
estimate, projected annual cost), and a footer total.
