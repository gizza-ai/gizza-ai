# budget-planner — competitor analysis (2026-07-20)

Scan done BEFORE implementation (create-next-tool step: competitor scan). Three real,
reachable competitor tools were reviewed (paraphrased only — no copy/branding reused):

1. **NerdWallet — 50/30/20 Budget Calculator** (nerdwallet.com/finance/learn/nerdwallet-budget-calculator)
   - Single input: monthly after-tax income. Outputs three dollar buckets (necessities /
     wants / savings+debt). States the percentages can be adjusted to fit. Provides
     guidance lists of what belongs in each bucket (housing, transport, insurance,
     groceries = needs; dining out, streaming = wants; retirement, debt payments = savings).
2. **Omni Calculator — 50/30/20 Rule Calculator** (omnicalculator.com/finance/50-30-20-rule)
   - Monthly after-tax income input; worked example $4,500 → $2,250 / $1,350 / $900.
     Also offers a reverse calculation (necessities budget → minimum required income).
     Category definitions ("almost always unavoidable" vs "not necessary").
3. **Ramsey Solutions — Budget Calculator (zero-based)** (ramseysolutions.com/budgeting/budget-calculator)
   - Zero-based method: income + expense categories; computes a "difference" metric
     (income − expenses) shown as overspent (red) / balanced (zero) / surplus (green).
     Publishes per-category advisory guidelines (housing ≤ 25% of take-home, giving 10%,
     retirement 15%).

## Table stakes → model fit

| Capability (table stake) | Seen at | Fit | Where it landed |
|---|---|---|---|
| Monthly after-tax (take-home) income input | all 3 | in-model | `income` (number, required) |
| Three-bucket 50/30/20 dollar split with percentages | NerdWallet, Omni | in-model | rule mode output (targets sum exactly to income; last bucket takes the rounding remainder) |
| Adjustable/custom percentages | NerdWallet | in-model | `split` param ("50/30/20" default; any three shares summing to 100, e.g. "60/30/10", "80/0/20") |
| Worked example $4,500 → 2,250/1,350/900 | Omni (+ backlog prompt) | in-model | default `[[example]]` chip + content.md worked example |
| What-counts-as-needs/wants/savings guidance | NerdWallet, Omni | in-model | page copy + FAQ (paraphrased, generic) |
| Zero-based: expense categories, income − planned difference | Ramsey | in-model | `mode=zero-based` + `expenses` list; `left_to_allocate` + surplus/deficit/balanced status |
| Per-category share of income | Ramsey (implicit) | in-model | share % per category row |
| Actual-vs-target per bucket (plan your listed expenses against the rule) | extension of all 3 | in-model | rule mode accepts bucket-tagged expenses → target vs planned vs left per bucket |
| Color-coded difference (red/green) | Ramsey | out-of-model (visual) | text report states `surplus` / `deficit` / `balanced` explicitly instead; page output is plain text |
| Reverse calculation (needs budget → required income) | Omni only | out-of-model | niche inverse; would fork the schema — documented here, not built |
| Advisory per-category thresholds by NAME (housing ≤ 25%, giving 10%…) | Ramsey | out-of-model | needs semantic classification of free-text category names; generic guidance paraphrased in FAQ instead |
| Pay-frequency conversion (annual/biweekly → monthly) | some variants | out-of-model | single monthly input (like all 3 top tools); FAQ documents the divide-by-12 conversion |
| Save/track budget over months (accounts, apps) | Ramsey/PocketGuard apps | out-of-model | stateless tool by design |

## UX control patterns observed → ours

- Single prominent income field with $ example values → number field, placeholder `4500`.
- Method choice → `mode` as `Param::enumv` (`50-30-20` | `zero-based`) with friendly
  `[input.labels]`.
- Preset ratios (50/30/20 is itself the preset; NerdWallet hints at variants) →
  `[[example]]` chips: 50/30/20 on $4,500; a 60/30/10 high-cost-of-living variant;
  a zero-based paycheck plan with categories.
- Expense entry is line-per-category in every zero-based tool → `multiline = true`
  textarea, `Name: amount (bucket)` lines, placeholders showing the exact format.

## Decisions

- Amounts are parsed to whole cents (accepts `$`, thousands separators); all arithmetic in
  integer cents so buckets/totals always reconcile to the penny.
- In rule mode, listed expenses MUST carry a bucket tag (`(needs)`/`(wants)`/`(savings)`) —
  a silent default would misclassify; the error names the offending line.
- Caps: ≤ 100 expense lines, income and each amount ≤ 1,000,000,000, category names ≤ 60
  chars — all stated on the page.
