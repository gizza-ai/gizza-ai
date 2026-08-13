# cartesian-to-polar-csv — competitor analysis (2026-08-13)

Scan run **before** implementation, per the create-next-tool recipe. All findings are
paraphrased observations of publicly documented feature sets; no competitor copy, branding,
or trademark text was copied into the block.

## Competitors reviewed

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | lddgo.net — Cartesian/Polar coordinate converter | batch textarea | Closest analogue: multi-line comma-separated pairs, both directions, downloadable results |
| 2 | dCode — 2D coordinates converter | single point | Cartesian ⇄ polar, results exportable as CSV or TXT, strong FAQ section |
| 3 | miniwebtool — Cartesian to Polar converter | single point | High decimal precision with quick-select presets, formula + quadrant explanation |
| 4 | Omnicalculator — Polar coordinates calculator | single point | Bi-directional, documents the θ ∈ (−π, π] range constraint |

## Table stakes observed → decision

| Capability | Seen on | In model? | Where it landed |
|---|---|---|---|
| Batch / many points at once | lddgo | yes | The whole premise — `csv` textarea, capped at 5 MB / 200,000 rows |
| Both directions (x,y → r,θ and r,θ → x,y) | lddgo, dCode, Omnicalculator | yes | `direction` enum (`cartesian_to_polar`, `polar_to_cartesian`) |
| Degrees vs radians | lddgo, miniwebtool | yes | `angle_unit` enum; gradians and turns added as a superset |
| Angle range choice — signed (−180, 180] vs positive [0, 360) | lddgo | yes | `angle_range` enum (`signed`, `positive`) |
| Decimal precision | lddgo (0–15), miniwebtool (1–1000) | partly | `decimals` 0–15 slider. f64 carries ~15–17 significant digits, so 1000-place output is **out of model** — arbitrary-precision arithmetic is not worth a bignum dependency here, and the limit is stated on the page |
| Downloadable / copyable results | lddgo, dCode | yes | Page uses `format = "text"`, which gets the generic Download link + copy affordance |
| CSV/TSV export shape | dCode | yes | `output` enum (`csv`, `tsv`, `json`, `table`) |
| Correct four-quadrant angle (atan2, not arctan) | miniwebtool, Omnicalculator | yes | Core uses `f64::atan2`; documented in the FAQ, with the quadrant caveat Omnicalculator's `arctan(y/x)` formula glosses over |
| Formula / explanation content | miniwebtool, dCode, Omnicalculator | yes | `content.md` states r = √(x²+y²), θ = atan2(y, x), x = r cos θ, y = r sin θ plus a worked example |
| Preset quick-picks | miniwebtool (precision chips) | yes | `[[example]]` preset chips on the page |
| Interactive coordinate-plane plot, step-by-step derivation | miniwebtool | no | **Out of model** — the page surface is a form + text output, with no plotting canvas. Listed, not built |
| File upload of a coordinate text file | lddgo | no | **Out of model for a pure tool** — pure blocks take text fields, not file inputs (file input would force a chat/CLI-only, page-less tool). Paste is the supported path; documented |

## Gaps ours closes that the competitors do not

None of the four handles a *real* CSV: headers, extra non-coordinate columns, quoted fields,
or a delimiter other than comma. Every one of them expects bare numeric pairs. So the block
adds, beyond table stakes:

- **Header-aware column selection** — `x_column` / `y_column` accept a header name *or* a
  1-based index, and auto-detect common names (`x`/`y`, `r`/`theta`, `rho`/`phi`, …) when left blank.
- **Passthrough of unrelated columns** — `keep_columns` retains `id`, `label`, timestamps etc.
  alongside the converted values, so the output is still a usable dataset.
- **Delimiter handling** — `delimiter` auto-sniffs comma / semicolon / tab / pipe and echoes the
  same delimiter on output.
- **Row-numbered errors** — a bad cell reports the offending row and value instead of failing silently.

## Out-of-model list (recorded, deliberately not built)

1. Arbitrary/1000-digit decimal precision (f64 caps at ~15–17 significant digits).
2. Interactive coordinate-plane visualization and animated quadrant diagrams.
3. Step-by-step symbolic derivation of each conversion.
4. Upload of a coordinate file from disk (pure blocks have no file input; paste instead).
5. 3D / cylindrical / spherical coordinate systems — a different tool, not a silent extension.
