# unit-converter — competitor analysis (2026-06-21)

New tool. Built pure-Rust (length, mass, temperature, volume, area, speed, data
size, time), all three surfaces verified: chat block (wafer-validated), CLI
(`gizza tool unit-converter value=… from=… to=…`), and standalone page
(Playwright).

## Surfaces verified

- **Chat / LLM API:** `wafer build` OK (301.7 KiB), drift-guard schema test passes
  (no LLM-facing schema drift). Params: `value` (number, required), `from`,
  `to` (strings, required).
- **CLI:** `gizza tool unit-converter value=5 from=km to=miles` →
  `"5 kilometre = 3.106855961187 mile"`; cross-category error path returns
  `cannot convert metre (length) to kilogram (mass)` with exit 1.
- **Page:** `/tools/unit-converter/` — 2 Playwright specs pass (km→miles,
  celsius→fahrenheit).

## Top competitors surveyed

1. **unitconverters.net** — broadest catalogue; dozens of categories incl.
   engineering (pressure, energy, power, force, fuel economy, etc.).
2. **calculator.net conversion calculator** — common categories + a clean
   dropdown UI, history of conversions.
3. **convertunits.com** — free-text "convert X to Y" parsing, very large unit set.
4. **onlineconversion.com** — temperature incl. Rankine and Réaumur; very wide
   long-tail unit coverage.
5. **omnicalculator conversion tools** — category-specific calculators, mobile
   friendly, scientific-grade precision (NIST/BIPM factors).

## Gap analysis vs. our build

| Capability | Competitors | gizza unit-converter | Status |
|---|---|---|---|
| Length / mass / temperature / volume / area / speed / time | all | yes | covered |
| Data size (decimal SI + binary IEC) | some | yes (KB↔KiB etc.) | covered (differentiator vs. simpler tools) |
| Symbol/alias + plural input (`m`/`metre`/`meters`) | most | yes | covered |
| Rankine + Réaumur temperature scales | onlineconversion | **added Réaumur** this build | closed |
| Exact internationally-defined factors (inch=2.54cm, lb=0.45359237kg) | best tools | yes | covered |
| Cross-category guard with helpful error | varies | yes (names both categories) | covered / better |
| Runs locally, nothing uploaded | a few | yes (browser-local wasm) | covered (privacy differentiator) |

## In-model gaps NOT built (candidate follow-ups, all pure-compute)

- **More categories**: pressure (Pa/bar/psi/atm), energy (J/cal/kWh/BTU), power
  (W/hp), force (N/lbf), frequency, angle (deg/rad), fuel economy (mpg/L·100km).
  All are linear (or simple) and would fit the same `Category`/`factor` model —
  deferred to keep this tool's scope to the 8 categories named in the backlog row.
- **One-click swap / live recompute**: a page-UX nicety; the page already
  recomputes on input. A dedicated swap button is a `site/tool.js` change, out of
  this tool's scope.

## Not applicable / out of model

- Currency conversion (needs live FX rates — network/ML, not pure-compute).
- Cooking ingredient-density conversions (g↔cup of flour) need a density table
  per ingredient — a distinct tool, not a generic unit converter.

## Copy / branding

No competitor copy, branding, or trademarks were used. All SEO copy, tool
description, and tag list are original.
