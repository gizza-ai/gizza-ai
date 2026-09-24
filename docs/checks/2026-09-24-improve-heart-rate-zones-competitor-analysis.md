# heart-rate-zones — competitor analysis (2026-09-24)

Scan of the real heart-rate-zone calculators people land on, used to shape the descriptor, the
page controls and the page copy for `blocks/heart-rate-zones`. Everything below is
**paraphrased** from public product surfaces; no competitor copy, branding, trademarks or assets
were reused.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | Omni Calculator — Karvonen formula calculator | The single-target version: resting HR, maximum HR (auto-filled from `220 − age`) and one intensity percentage in, one target heart rate plus the heart-rate reserve out. Share/reload controls, two FAQ entries, heavy cross-linking to sibling calculators. |
| 2 | Topend Sports — Karvonen heart rate calculator | Athlete-facing five-zone table (recovery / aerobic base / aerobic threshold / anaerobic / maximum) with a purpose column, resting HR defaulted to 70, optional age for `220 − age`, athlete-category presets (elite / trained / average / beginner), print + share, and sport-specific guidance including the 80/20 rule and a ±5–10 bpm heat/altitude note. |
| 3 | Omni Calculator — heart rate zone calculator | The formula-choice version: age, optional resting HR, a training-aim dropdown, and a maximum-HR method selector offering several published fits (Haskell & Fox, Inbar, Nes, a nonlinear Oakland-style fit, Tanaka) plus a custom value. Renders the standard five zones. |
| 4 | runbundle — heart rate zones calculator | Method toggle between percent-of-maximum and heart-rate reserve, maximum and resting HR as direct bpm inputs, a five-band table (light / aerobic / high aerobic / anaerobic / red line) showing **both** the %MHR and %HRR columns side by side, plus separate published fat-burning bands for men and women and a references section. |
| 5 | Calculate My Heart Rate / Healthy Life Scale / Year Round Running class of pages | The commodity tier: age + resting HR in, five Karvonen zones out, long SEO copy, zone-2 and fat-burning explainers, no method or model choice. |

## Table-stakes checklist

| Capability | Competitors that ship it | Our decision |
|---|---|---|
| Age input | 1, 2, 3, 5 | **in-model — descriptor** (`age`, 5–120, slider; only used when `max_hr` is 0) |
| Resting heart rate input | all | **in-model — descriptor** (`resting_hr`, 25–120, slider, default 60) |
| Measured maximum HR overriding the formula | 1, 2, 3, 4 | **in-model — descriptor** (`max_hr`, 0 = estimate from age; rejected when at or below resting) |
| Karvonen / heart-rate-reserve method | all | **in-model — descriptor** (`method=karvonen`, the default) |
| Plain percent-of-maximum method | 3, 4 | **in-model — descriptor** (`method=percent-max`) |
| Choice of age formula | 3 | **in-model — descriptor**, widened this pass from 4 to 6: `tanaka`, `fox`, `gulati`, `nes`, `inbar` (205.8 − 0.685 × age) and the nonlinear `oakland` (192 − 0.007 × age²), matching competitor 3's breadth |
| Five-zone table with names + training purpose | 2, 3, 4, 5 | **in-model — descriptor/page** (`model=five-zone`, with a focus column per zone) |
| Single target heart rate from one intensity % | 1 | **in-model — descriptor** (`intensity`, 30–100, slider; 0 omits the line) |
| Heart-rate reserve reported explicitly | 1, 4 | **in-model — descriptor/page** (its own line in every output mode) |
| Athlete-category presets for resting HR | 2 | **in-model — page**, as `[[example]]` chips (average adult, trained runner with a measured max, elite endurance profile) rather than a preset dropdown that would overwrite a typed value |
| Both %MHR and %HRR columns shown together | 4 | **considered, rejected** — one method per run keeps the output unambiguous about which number to train by; the page copy states the ~15–20 bpm offset and the two chips let you flip between them in one click |
| Gender-specific fat-burning bands | 4 | **considered, rejected** — the published men/women bands differ by less than the ±7–12 bpm error in an age-estimated maximum, so the precision would be fake. A FAQ entry says where the fat-burning zone actually sits (zone 2) and why the label oversells it |
| Three-zone polarized model | — (coaching sites, not these calculators) | **in-model — descriptor** (`model=three-zone`), a genuine differentiator here |
| Zoladz fixed-offset bands | — | **in-model — descriptor** (`method=zoladz`), likewise |
| AHA moderate/vigorous public-health bands | — | **in-model — descriptor** (`model=aha`) |
| Machine-readable / pasteable output | — | **in-model — descriptor** (`output=summary\|table\|json`; markdown tables for training plans, JSON for scripts) |
| 80/20 and sport-application guidance | 2 | **in-model — page**, in the three-zone copy and the FAQ (original wording) |
| Heat / altitude / illness caveat | 2 | **in-model — page**, in the limits section |
| Beta-blocker / cardiac caveat | 1 (generic medical disclaimer) | **in-model — page**, as a specific FAQ rather than a blanket disclaimer |
| Print / share buttons | 1, 2 | **out-of-model as buttons** — the page ships shareable deep links (every field is a query param) and the generic copy/download control instead of bespoke chrome |
| Lactate-threshold (LTHR / Friel) zone tables | running-coach sites | **considered, rejected** — needs a separate measured-threshold input and a different 5–7 band table; the measured-`max_hr` path already addresses the accuracy motivation, and the page names LTHR zones as out of scope |
| Zone bar chart / gauge | 3, 5 | **out-of-model for this page** — the generic tool-page renderer outputs text or media, not plots; the table output is the paste-ready equivalent |
| Accounts, saved athlete profiles, watch sync (Garmin/Polar/Strava) | vendor apps | **out-of-model** — browser-local, no backend, no accounts |
| Ads / related-calculator cross-sell | 1, 3, 5 | **out-of-model** — not a product feature |

## UX / controls observed and what we did

- Competitors use plain number boxes for age, resting HR and intensity. Ours are **sliders paired
  with the canonical number box** (`kind = "slider"`), because all three are bounded ranges people
  nudge rather than type once.
- Competitor 3's value is its **method selector**; every fixed choice here is a `Param::enumv`, so
  the page renders real `<select>` controls, and `[input.labels]` spells each formula out
  (`Tanaka — 208 − 0.7 × age`) instead of showing bare slugs.
- Competitor 2 ships athlete-category presets; we ship **one-click example chips** covering an
  average adult, a trained runner with a measured maximum, a polarized three-zone athlete, an
  elite endurance profile, Zoladz bands as a table, and AHA bands as JSON.
- Competitors report zones as a bare bpm range. Every row here also carries **what the zone is
  for**, and the header states **where the maximum came from** (measurement vs the named formula),
  which is the single most common source of "why doesn't this match my watch?".
- Errors name the expected range and the value received (`expected age between 5 and 120 years,
  got 2`) rather than failing silently or clamping.

## Non-goals recorded

Lactate-threshold and power-based (FTP) zones, VO2 max estimation, training-load or TRIMP scoring,
recovery-rate analysis, and any device sync are outside this tool's model and are stated as such
rather than approximated.
