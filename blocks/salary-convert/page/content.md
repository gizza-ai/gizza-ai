# Salary Converter

Convert a pay figure between **hourly, daily, weekly, biweekly, monthly and annual**
rates. Enter one amount, pick the period it's quoted in, set your work schedule, and get
every equivalent figure back at once.

## What it does

- `$25/hour` at 40 hours a week and 52 weeks a year becomes `$52,000/year`.
- `$52,000/year` becomes `$25.00/hour`, `$1,000/week`, `$4,333.33/month`.
- Works from **any** period — hourly, daily, weekly, biweekly, monthly or annual — to all the others.
- Adjust **hours per week**, **days per week**, and **paid weeks per year** to match real schedules (part-time, unpaid leave, a 37.5-hour week).
- All figures are **gross pay, before tax** — this is a rate converter, not a paycheck/net-pay calculator.

## How the conversion works

Everything pivots through an annual figure and an explicit work schedule:

- **annual = hourly × hours-per-week × weeks-per-year** (e.g. 25 × 40 × 52 = 52,000)
- **hourly = annual ÷ (hours-per-week × weeks-per-year)**
- **weekly = annual ÷ weeks-per-year**
- **biweekly = annual ÷ 26** (annual ÷ (weeks-per-year ÷ 2))
- **monthly = annual ÷ 12** — *not* weekly × 4
- **daily = annual ÷ (days-per-week × weeks-per-year)**

The defaults (40 hours/week, 5 days/week, 52 weeks/year) give the standard 2,080-hour work year;
change any of them for part-time, compressed, or PTO-adjusted schedules.

## Inputs

- **Pay amount** — a plain number; currency symbols and thousands separators are ignored.
- **This amount is per** — the period your amount is quoted in.
- **Hours per week** — default 40. Drives the hourly figure.
- **Days per week** — default 5. Drives the daily figure.
- **Paid weeks per year** — default 52. Lower it to model unpaid time off (e.g. 50 for two unpaid weeks).
- **Currency symbol** — cosmetic; shown in the summary text only, it never changes the numbers.

Everything runs locally in your browser; no pay data leaves your machine.

## FAQ

<details>
<summary>$25 an hour is how much a year?</summary>

At the standard full-time schedule — 40 hours a week for 52 weeks — `$25/hour` works out to
`25 × 40 × 52 = $52,000` a year, which is about `$4,333.33` a month and `$1,000` a week. Change
the hours-per-week or weeks-per-year fields if your schedule differs.

</details>

<details>
<summary>Why is monthly pay not just weekly pay times four?</summary>

Because a year has about 4.33 weeks per month, not 4. Multiplying weekly pay by 4 undercounts by
roughly a month's pay over the year. This converter always computes **monthly = annual ÷ 12**, so a
`$1,000`/week salary shows as `$4,333.33`/month, not `$4,000`.

</details>

<details>
<summary>How do I account for unpaid time off?</summary>

Lower the **paid weeks per year**. The default 52 assumes every week is paid. If you take two
unpaid weeks, set it to 50; the annual and per-period figures drop accordingly while your hourly
rate stays the same.

</details>

<details>
<summary>What's the difference between biweekly and semi-monthly?</summary>

**Biweekly** means every two weeks — 26 paychecks a year (annual ÷ 26), which this tool shows.
**Semi-monthly** means twice a month — 24 paychecks a year (annual ÷ 24). They're close but not
equal; biweekly paychecks are slightly smaller because there are more of them.

</details>

<details>
<summary>Are these figures before or after tax?</summary>

Before tax. Every figure is **gross pay** — your actual take-home depends on income tax, national
insurance / payroll tax, pension or retirement contributions, and other deductions that vary by
country and personal situation. Use this to compare rates and offers, not to predict a net paycheck.

</details>
