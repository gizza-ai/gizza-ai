# interpolation — competitor analysis (2026-08-29)

Scan run **before** implementing `blocks/interpolation`, to set the table stakes the descriptor
had to cover on day one. All observations are **paraphrased** from public product pages; no
competitor copy, branding, or trademark is reproduced here or in the tool.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | linearinterpolationcalculator.com | General-purpose interpolation calculator: point table, three methods, chart, CSV in/out |
| 2 | enghandbook.com/calculators/interpolation | Engineering-handbook calculator: point table, three methods, comparison chart |
| 3 | atozmath.com/CONM/CubicSpline.aspx | Cubic-spline solver with step-by-step working, piecewise equations, y and y′ evaluation |

(Also seen in the result set but not profiled in depth: hvks.com's Plotly-based polynomial/spline
page and agentcalc.com's cubic-spline page, which is where the "evaluate at x (comma-separated)"
field wording pattern shows up.)

## Table stakes observed

| Capability | 1 | 2 | 3 | Our decision |
|---|---|---|---|---|
| Enter (x, y) data points | ✅ | ✅ | ✅ | **In-model** — `data`, one `x,y` per row, plus space/tab/semicolon rows, a bare y list (x = 1,2,3…), JSON pairs/objects |
| Linear interpolation | ✅ | ✅ | — | **In-model** — `method=linear` |
| Cubic-spline interpolation | ✅ | ✅ | ✅ | **In-model** — `method=cubic` |
| Polynomial (single curve through all points) | ✅ | ✅ | — | **In-model** — `method=polynomial`, barycentric Lagrange, capped at 30 points |
| Evaluate at one or more x values | ✅ | ✅ | ✅ | **In-model** — `at`, comma/space/semicolon-separated list (competitor 1 and 2 accept one x; we accept many) |
| Derivative value at x (y′) | — | — | ✅ | **In-model** — `derivative` = 0, 1 or 2 |
| Piecewise coefficients / segment equations | ✅ | ✅ (linear + poly) | ✅ | **In-model** — `coefficients` boolean, printed as readable `y = a + b(x−xi) + …` segment equations plus coefficient rows |
| Spline end conditions | — | — | natural only | **In-model** — `boundary` = natural / not-a-knot / clamped, with `start_slope` / `end_slope` for clamped (MATLAB's `spline` defaults to not-a-knot, atozmath fixes M₀=M₃=0, so both conventions are worth offering) |
| Decimal-place control | — | — | ✅ (0–10) | **In-model** — `decimals`, 0–12, default 6 |
| Chart of points + fitted curve | ✅ | ✅ | — | **In-model as a static chart** — `output=svg` renders a self-contained SVG of the data points and the interpolant. Hover tooltips / pan-zoom are out-of-model (no JS charting library in a wasm page) |
| CSV import / export | ✅ | — | — | **In-model as text** — CSV text pastes straight into `data`; `output=csv` returns CSV. A file-picker import button is out-of-model for a field-input page |
| Preset example datasets | ✅ (linear / quadratic / oscillating) | — | ✅ (4 worked problems) | **In-model** — three `[[example]]` preset chips on the page |
| Reset button | ✅ | — | — | **In-model** — the generator gives every field page Reset + Copy for free |
| Duplicate-x rejection | ✅ | ✅ ("each x unique and rising") | — | **In-model** — duplicate x is a named error; unsorted input is sorted rather than rejected (strictly better than competitor 2, which demands ascending order) |
| Minimum-point rules per method | ✅ | ✅ | — | **In-model** — enforced with a message that names the method and the count needed |
| Step-by-step tridiagonal working | — | — | ✅ | **Considered, not built** — a solver trace is a teaching feature, not a data feature; the segment equations already let a reader check the result. Listed, not dropped silently |

## Gaps we close that none of the three cover

- **Extrapolation policy** (`extrapolate` = error / clamp / extend). Every competitor is silent about
  what happens outside the data range; ours names the behavior and defaults to refusing.
- **Monotone (PCHIP) interpolation** (`method=monotone`) — a shape-preserving cubic that cannot
  overshoot between points, which is what you want for cumulative or physically-bounded data.
- **Nearest-neighbour** (`method=nearest`) for lookup-table / step-function data.
- **Resampling** (`resample = N`) — N evenly spaced samples across the data range in one call,
  instead of typing every x by hand.
- **JSON output** with the full report (method, segments, evaluations, warnings), for scripting.
- **CLI + chat surfaces** — the same interpolant from `gizza tool interpolation …`, not only a page.

## Out-of-model (listed, not built)

| Feature | Why it does not fit |
|---|---|
| Interactive hover/zoom chart (Plotly-style) | Needs a JS charting runtime; our pages render a static self-contained SVG instead |
| CSV file-upload button | The page's input is a text field; pasting the same CSV text is the equivalent path |
| Print/save-as-PDF of the working | Browser print already covers it; no tool-side feature needed |
| Step-by-step elimination narration | Teaching output, not data output — see the table above |
| 3D / bivariate (surface) interpolation | A different input shape (grid) and a different tool; would not fit this descriptor |
| Accounts, saved datasets, shared links beyond query params | No backend, no accounts — outside the model |

## Copy / UX patterns adopted (as patterns, in our own words)

- Explicit per-method minimum point counts stated on the page, not just raised as an error.
- Preset chips for a linear, a curved and an oscillating dataset, so the page shows a result on
  first click.
- A Runge's-phenomenon note in the FAQ explaining why high-degree polynomial fits oscillate and
  when to prefer a spline — a real question the competitor FAQs also field, answered originally.
- Stated precision behavior: the maths runs at full `f64` precision and only the printed values are
  rounded to `decimals`.
