## About this tool

The **returns risk analyzer** turns a plain series of periodic returns into the
performance and risk figures you'd otherwise assemble in a spreadsheet:

- **Return:** per-period mean, total (cumulative) return, and the **geometric,
  compound annualized return** (CAGR-style) for the chosen frequency.
- **Risk:** **annualized volatility** (the sample standard deviation scaled by
  √periods) and **downside deviation** below your target.
- **Risk-adjusted ratios:** **Sharpe** (excess return over the risk-free rate ÷
  volatility), **Sortino** (excess return over the target ÷ downside deviation),
  and **Calmar** (annualized return ÷ max drawdown).
- **Drawdown & shape:** **max drawdown** of the compounded equity curve, best and
  worst single period, and the share of positive periods.

Paste one return per line, or separate them with commas or spaces. Each value can
be a **decimal** (`0.012`) or a **percent** with a `%` sign (`1.2%`) — mix them
freely. Pick the **frequency** that matches your data so annualization uses the
right factor: daily = 252, weekly = 52, biweekly = 26, monthly = 12, quarterly =
4, annual = 1.

### Worked example

For 12 monthly returns `0.021, -0.008, 0.015, 0.033, -0.012, 0.004, 0.019,
-0.006, 0.011, 0.027, -0.021, 0.014` with **frequency = monthly (12)** and a **2%
risk-free rate**, the tool reports a cumulative return of about **9.98%**, an
annualized return near **9.98%**, annualized volatility around **6.06%**, and a
Sharpe ratio of roughly **1.28** — with the exact figures shown in the result box.

### Conventions

Volatility uses the **sample** standard deviation (÷ n−1); downside deviation
divides by **n** (population) over all observations. Annualized return is
**geometric** (compound). Sharpe uses the **risk-free rate**; Sortino uses the
**target return** as the minimum acceptable return (MAR). Both ratios are
annualized by √periods. These are stated so your numbers are reproducible — other
calculators sometimes leave them unspecified.

You need **at least 2 returns**. This is an educational calculator, **not
financial advice**.

### Privacy

Everything runs **in your browser** via WebAssembly — your returns are never
uploaded. Also available from the [gizza CLI](/) and in chat (which return the
values as structured JSON).

## FAQ

<details>
<summary>Should I enter returns as 0.012 or 1.2%?</summary>

Either — the tool accepts both, per value. A bare number is read as a decimal
(`0.012` = 1.2%), and a number with a `%` sign is divided by 100 (`1.2%` =
0.012). You can mix the two in one series. What you must not do is enter `1.2`
meaning 1.2%: without the `%` sign that is read as +120%.

</details>

<details>
<summary>Which annualization factor should I choose?</summary>

Match it to how often your returns are sampled: **daily** trading returns use
252, **weekly** 52, **biweekly** 26, **monthly** 12, **quarterly** 4, and
already-annual figures use 1. The factor scales the mean return and (via its
square root) the volatility, so the wrong choice throws off every annualized
number.

</details>

<details>
<summary>Why are my Sharpe and Sortino different from another calculator?</summary>

Small differences usually come from undocumented conventions. Here, volatility
uses the sample standard deviation (÷ n−1), downside deviation divides by n over
all periods, the annualized return is geometric, and both ratios are annualized
by √periods. A tool that uses population standard deviation, a different downside
divisor, or an arithmetic annualized return will land on slightly different
values — none is "wrong", they just answer with different definitions.

</details>

<details>
<summary>What do "undefined" Sharpe or Sortino mean?</summary>

The **Sharpe** ratio is undefined when volatility is zero (every return is
identical, so there's nothing to divide by). The **Sortino** ratio is undefined
when no return falls below your target, so there's no downside deviation. The
tool says *undefined* with the reason rather than printing infinity.

</details>

<details>
<summary>How many returns do I need, and how reliable is the result?</summary>

You need at least 2 to have any dispersion to measure. As with any statistic,
short series give noisy, unreliable risk figures — a handful of points can't
characterize a distribution, so treat results from small samples with caution.

</details>
