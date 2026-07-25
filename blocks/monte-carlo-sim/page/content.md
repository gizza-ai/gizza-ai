## About this tool

A **Monte Carlo simulation** answers "what's likely to happen when several inputs are uncertain?" Instead of a single point estimate, you describe each unknown as a *probability distribution*, combine them in a formula, and draw thousands of random trials. The spread of outcomes tells you not just the average, but the risk: the 10th percentile, the median, the 90th percentile, and the probability of clearing a target.

This tool runs entirely in your browser. You declare **input variables** — one `name = distribution(args)` per line — write a **model formula** that combines them, choose how many **trials** to run, and get back:

- summary stats (mean, standard deviation, min, max);
- percentiles **P5, P10, P25, P50 (median), P75, P90, P95, P99** — read these as confidence levels, e.g. "there's a 90% chance the cost is at or below the P90 value";
- an optional **probability of meeting a target** (set a threshold and pick at-or-above or at-or-below);
- a text **histogram** of the outcome distribution.

Runs are **deterministic**: the same inputs and `seed` always produce the same numbers, so results are reproducible and shareable. Change the seed to draw a different random sample.

### Supported distributions

| Distribution | Syntax | Use for |
|---|---|---|
| Normal (Gaussian) | `normal(mean, std_dev)` | symmetric noise around a central value |
| Uniform | `uniform(min, max)` | equally-likely anywhere in a range |
| Triangular | `triangular(min, mode, max)` | three-point estimates (worst / likely / best) |
| Lognormal | `lognormal(mu, sigma)` | positive, right-skewed quantities (durations, prices) |
| Constant | `constant(value)` | a fixed input |

### Worked example

Suppose revenue is roughly `normal(1000, 200)` and cost is anywhere in `uniform(400, 700)`. To model profit:

```
variables:
  revenue = normal(1000, 200)
  cost = uniform(400, 700)
model:
  revenue - cost
threshold: 0   (direction: at-or-above)
```

With 20,000 trials the report shows the mean profit (~450), the full percentile spread, `P(outcome >= 0)` — your probability of turning a profit — and a histogram of the profit distribution. Lower the number of trials to iterate quickly; raise it to tighten the percentile estimates.

The **model formula** supports `+ - * / ^`, parentheses, common functions (`sin`, `cos`, `sqrt`, `abs`, `ln`, `exp`, `min`, `max`, …) and constants (`pi`, `e`) — the same expression engine as a scientific calculator. Every name in the formula must be a declared variable.

## FAQ

<details>
<summary>What is a Monte Carlo simulation, in plain terms?</summary>

It's a way to estimate a range of outcomes when your inputs are uncertain. Rather than plugging in one "best guess" for each input, you describe each as a probability distribution, then let the computer draw a random value for each input thousands of times and record the result each time. The collection of results approximates the real distribution of outcomes, so you can read off averages, percentiles, and the chance of hitting a target.

</details>

<details>
<summary>How many trials should I run?</summary>

More trials give smoother, more stable percentiles but take longer. A few thousand is enough to see the shape; **10,000** (the default) is a good balance; 50,000–100,000 tightens the tail percentiles (P95, P99) when you need them precise. The range allowed here is 100 to 1,000,000.

</details>

<details>
<summary>What do the percentiles mean?</summary>

A percentile is the value below which that share of outcomes fall. **P90 = 1,200** means 90% of simulated outcomes were at or below 1,200. Project and finance teams often quote these as confidence levels — a "P80 budget" is the amount you'd exceed only 20% of the time. Use the target-probability field to compute the confidence for a specific number instead.

</details>

<details>
<summary>Are the results the same every time?</summary>

Yes, for a given **seed**. The simulation uses a deterministic pseudo-random generator, so identical inputs and seed produce byte-identical results — handy for sharing or documenting a run. Change the seed to draw a fresh independent sample and see how much the answer wobbles.

</details>

<details>
<summary>Which distribution should I pick?</summary>

Use **triangular(min, mode, max)** when you have a worst-case, most-likely, and best-case estimate (classic three-point project estimation). Use **normal(mean, std_dev)** for symmetric measurement noise. Use **lognormal** for quantities that can't go negative and are right-skewed, like task durations or prices. Use **uniform(min, max)** when anything in a range is equally plausible, and **constant** for a fixed value.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The simulation runs locally in your browser via WebAssembly — your variables and formula never leave your device, and there's no account or server round-trip.

</details>

<details>
<summary>What isn't supported?</summary>

This is a focused, single-formula simulator. It doesn't model correlations between inputs (each variable is sampled independently), fit distributions from a dataset, or produce tornado / sensitivity charts. Inputs are assumed independent; if two of your variables move together, the result will understate that dependence.

</details>
