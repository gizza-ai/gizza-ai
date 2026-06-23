# geometry-calculator — competitor analysis (2026-06-23)

New tool built this run; this snapshot records the competitor landscape that
shaped the descriptor + page, the gap analysis, and what was deliberately left
out (out-of-model). No competitor copy, branding, or assets were reproduced —
only feature/UX ideas were studied; all copy here and on the page is original.

## Tool under analysis

`geometry-calculator` — pick a shape, enter its dimensions, get area + perimeter
(2D) or surface area + volume (3D). 14 shapes, pure Rust, browser-local, no
account. Output is a structured JSON object (shape, dimensionality, echoed
dimensions, measures with `unit` suffix, human summary), shared across chat /
CLI / page.

## Competitors studied (function, not copied)

1. **CalculatorSoup — Geometry calculators.** Per-shape pages (one calculator per
   shape) covering 2D area/perimeter and 3D surface-area/volume. Each shape page
   solves for several unknowns (e.g. circle from radius/diameter/circumference/
   area). Strong shape coverage; SEO is per-shape, not a single combined tool.
2. **Omni Calculator — Area / Volume / Surface-area calculators.** Large family of
   focused calculators; clean, explains the formula, shows units, lets you change
   the unit. Each is a single-shape page with multiple solve-for modes.
3. **GoodCalculators / Calculator.net area & volume calculators.** Combined "area
   calculator" page with a section per common shape (square, rectangle, triangle,
   circle, trapezoid, ellipse, sector, parallelogram); separate volume calculator
   with sphere/cone/cube/cylinder/capsule/etc. Unit dropdowns.
4. **Symbolab / Mathway geometry.** Step-by-step solvers; take a typed problem and
   return worked steps. Strength is the explanation, not a dimension form.
5. **Inch Calculator — area/volume.** Shape picker + dimension inputs + unit
   selector, returns area/volume and shows the formula used. Closest in shape to
   our single combined picker tool.

## Gap analysis (fit-to-model)

| Capability | Competitors | gizza now | Decision |
|---|---|---|---|
| Many 2D shapes (square/rect/triangle/circle/ellipse/trapezoid/parallelogram/polygon) | yes | yes (8) | **in-model — shipped** |
| Many 3D shapes (cube/box/sphere/cylinder/cone/pyramid) | yes | yes (6) | **in-model — shipped** |
| Area + perimeter for 2D, surface area + volume for 3D | yes | yes | **in-model — shipped** |
| Triangle perimeter from the three sides (with validity check) | some | yes (triangle inequality enforced) | **in-model — shipped** |
| Regular-polygon area + perimeter from side count + edge | some | yes | **in-model — shipped** |
| Ellipse perimeter (no closed form) | a few | yes (Ramanujan approx., documented) | **in-model — shipped** |
| Shape-name aliases (box=cuboid=rectangular prism, oval=ellipse) | n/a | yes | **in-model — shipped** |
| Echo inputs + machine-readable JSON output (good for the LLM/chat surface) | rare | yes | **in-model — shipped (differentiator)** |
| Combined single-page shape picker (vs one page per shape) | mixed | yes | **in-model — shipped** |
| Solve-for-the-unknown (e.g. circle from area → radius) | several | no — forward direction only | **considered, not built this pass**; a future `solve_for` mode could invert each formula. Listed, not forced in. |
| Per-shape diagram / live drawing | some | no | out-of-model for a pure-compute text tool; the JSON + summary cover the numbers. |
| Physical unit dropdown + unit conversion (cm/in/ft…) | yes | no — unit-agnostic (results follow the input unit, documented) | **considered, not built**; pairs better with the dedicated `unit-converter` tool than duplicating it here. |
| Step-by-step worked solution | Symbolab/Mathway | no | out-of-model (that is a CAS/teaching product, not a calculator). |

## Outcome

Every in-model capability from the competitor set (shape breadth across 2D + 3D,
the right measures per dimensionality, polygon/ellipse handling, validity checks,
aliases) is in the shipped tool. The structured JSON + chat/CLI/page parity is a
gizza-specific differentiator most competitors lack. Out-of-model / deferred:
inverse "solve-for" modes, live diagrams, a physical-unit dropdown, and
step-by-step explanations — recorded here, not built.

## Verified surfaces

- **Unit + drift-guard:** `cargo test --workspace` — 19 core tests + the
  `schema_json_matches_authored_chat_schema` drift guard, all green.
- **Chat block:** `wafer build` validates `target/block.wasm` (337 KiB, instantiates).
- **CLI:** `gizza tool geometry-calculator shape=circle radius=2`,
  `shape=rectangular_prism width=2 height=3 length=4`, plus missing-dimension and
  unknown-shape error paths.
- **Page (Playwright, 4 tests):** circle (2D), box (3D), the `?shape=sphere&radius=3`
  query-param deep-link, and the empty-state prompt — all pass.
