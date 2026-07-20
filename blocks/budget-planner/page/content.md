## About this tool

The **Budget Planner** turns your monthly take-home pay into a concrete plan, two ways:

- **50/30/20 rule** — splits income into three target buckets: **needs** (rent, groceries,
  utilities, insurance, minimum debt payments), **wants** (dining out, streaming,
  entertainment, hobbies), and **savings** (emergency fund, retirement, extra debt payoff).
  The classic split is 50% / 30% / 20%, and you can change it to anything that sums to
  100 — `60/30/10` for a high-cost-of-living city, or `80/0/20` for a simple two-bucket plan.
- **Zero-based budget** — lists your expense categories against your income and shows the
  exact amount **left to allocate**. The goal: income minus planned spending equals zero,
  so every dollar has a job before the month starts.

All arithmetic is exact to the cent — bucket targets always add up to your income, with no
rounding drift. Everything runs locally in your browser; your income and spending never
leave your device.

### Worked example — 50/30/20

Income `4500` (monthly take-home), default split:

```
50/30/20 budget · take-home income $4,500.00/month

Bucket   Share     Target
Needs      50%  $2,250.00
Wants      30%  $1,350.00
Savings    20%    $900.00
```

Half of $4,500 goes to needs ($2,250), 30% to wants ($1,350), and 20% to savings ($900).
Add bucket-tagged expenses (e.g. `Rent: 1200 (needs)`) and each bucket also shows what you
have **planned** against its target and what's left — with `(over)` flagged when a bucket
exceeds its target.

### Worked example — zero-based

Income `2500`, expenses `Rent: 1400`, `Groceries: 450`, `Utilities: 180`, `Fun money: 200`:

```
Zero-based budget · take-home income $2,500.00/month

Category            Planned  Share
Rent              $1,400.00  56.0%
Groceries           $450.00  18.0%
Utilities           $180.00   7.2%
Fun money           $200.00   8.0%
Total planned     $2,230.00  89.2%
Left to allocate    $270.00  10.8%

Assign the remaining $270.00 — a zero-based budget gives every dollar a job.
```

If you plan more than you earn, the report says how far **over budget** you are; when income
minus planned spending is exactly zero, it confirms every dollar is assigned.

### Writing the expense list

One category per line (or separated by `;`):

```
Rent: 1200 (needs)
Groceries: $1,050.50 (needs)
# comment lines starting with # or // are skipped
Streaming: 30 (wants)
Emergency fund: 300 (savings)
```

Amounts may include a currency symbol and thousands separators. In **50/30/20 mode** every
line needs a bucket tag — `(needs)`, `(wants)` or `(savings)` (aliases like `need`, `want`,
`save`, `debt` work too). In **zero-based mode** tags are optional.

### Limits

Up to **100 expense lines**, category names up to **60 characters**, and income/amounts up
to **1,000,000,000** with at most **2 decimal places**. Negative amounts are rejected — model
income reductions by lowering the income figure instead.

## FAQ

<details>
<summary>Should I enter gross or take-home income?</summary>

Take-home (after-tax) income — the amount that actually lands in your account after taxes
and payroll deductions. Budgets built on gross income overstate what you can spend. If you
only know your annual salary, divide the after-tax total by 12 to get the monthly figure.

</details>

<details>
<summary>What counts as a need, a want, and savings?</summary>

**Needs** are expenses you can't reasonably avoid: housing, utilities, groceries, transport,
insurance, childcare, and minimum debt payments. **Wants** are lifestyle choices: dining out,
streaming, hobbies, travel, upgrades. **Savings** covers your emergency fund, retirement
contributions, and any debt payments beyond the minimums. If a line could be either, ask
whether next month would break without it — that's the test for a need.

</details>

<details>
<summary>What's the difference between the 50/30/20 rule and zero-based budgeting?</summary>

The 50/30/20 rule is top-down: it hands you three target amounts and leaves the details
inside each bucket up to you — fast, and good for a first budget. Zero-based budgeting is
bottom-up: you list every category yourself and keep allocating until income minus planned
spending equals zero, which gives complete control but takes more upkeep. This tool does
both, so you can start with the rule and graduate to zero-based.

</details>

<details>
<summary>Can I change the 50/30/20 percentages?</summary>

Yes. Put any three shares that add up to 100 in the split field — `60/30/10` when housing
eats more of your income, `70/20/10` for aggressive fixed costs, or `80/0/20` for a simple
two-bucket essentials/savings plan. Separators can be `/`, commas or spaces, and decimals
like `33.3/33.3/33.4` are fine. The split is ignored in zero-based mode.

</details>

<details>
<summary>Why does the tool insist on bucket tags in 50/30/20 mode?</summary>

Because guessing would misclassify your money. To compare your planned spending against the
needs/wants/savings targets, each expense must say which bucket it belongs to — end the line
with `(needs)`, `(wants)` or `(savings)`. If a tag is missing, the error names the exact
line so you can fix it. In zero-based mode no tags are needed.

</details>

<details>
<summary>Is my financial data uploaded anywhere?</summary>

No. The planner is WebAssembly running entirely in your browser — income and expenses are
processed locally and never sent to a server, so it's safe for real household numbers.

</details>
