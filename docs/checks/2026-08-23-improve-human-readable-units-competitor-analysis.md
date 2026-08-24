# human-readable-units competitor analysis (2026-08-23)

Picked tool: `human-readable-units` — formats raw byte counts, durations and large numbers into readable units (`1.5 GB`, `2m 3s`, `1.2M`).

## Sources scanned

- Common byte-format snippets and library docs: JavaScript format-bytes snippets, Perl Number::Bytes::Human and Java byte-size formatter writeups.
- Browser size converter pages: byte-count to readable text tools that expose a single input and byte-scale selection.
- Duration formatter tools: Browserling seconds-to-human and duration formatter pages that convert seconds/milliseconds into compact, long, clock and ISO forms.

## Table-stakes features and model-fit decisions

| Capability / UX pattern | In model? | Decision |
| --- | --- | --- |
| Single numeric input with examples such as `1500000000`, `123` and `1200000` | Yes | `value` is the required input; example chips cover bytes, duration and large-number cases. |
| Accept grouped numbers and scientific notation | Yes | Core parser accepts commas, underscores, spaces and `1.5e9` style input. |
| Decimal byte scale (`kB`, `MB`, `GB`) | Yes | `base=decimal`, default. |
| Binary byte scale with IEC suffixes (`KiB`, `MiB`, `GiB`) | Yes | `base=binary-iec`. |
| Binary 1024 scale with legacy `KB`/`MB` labels | Yes | `base=binary-jedec`. |
| Precision control for scaled bytes and counts | Yes | `precision` integer/slider, range `0..6`. |
| Trim or keep fixed trailing zeros | Yes | `trim_zeros` checkbox; page and CLI tests cover non-default false. |
| Compact, long-word and no-space output styles | Yes | `style=short|long|narrow`. |
| Convert durations from seconds, milliseconds, minutes/hours/days | Yes | `kind=duration` plus `input_unit` enum. |
| Duration output as compact text, long text, clock and ISO 8601 | Yes | `detail=breakdown` includes all alternates; primary output stays compact or long depending on `style`. |
| Plain large-number abbreviations (`K`, `M`, `B`, `T`) | Yes | `kind=number`. |
| Reverse parsing from human text back to raw units | Out of model for this build | Useful, but the backlog row asks for formatting raw units. Reverse parsing would require an additional grammar and ambiguity rules (`M` as mega vs minutes) better suited to a separate tool. |
| Batch column conversion | Out of model for this build | Existing gizza tools handle tabular transforms separately. This block intentionally formats one magnitude per invocation so chat, CLI and page stay simple and deterministic. |
| Locale-specific decimal/group separators and translated unit names | Out of model for this build | The repo's current generic page model has no locale selector. English symbols/words and common pasted separators are included. |

## Descriptor/page implications

- Use enum selects for `kind`, `input_unit`, `base`, `style` and `detail` so page controls do not render as free text.
- Use sliders for bounded numeric fields (`precision`, `max_units`) and example chips for the three headline examples plus binary and breakdown variants.
- Keep `trim_zeros` as a default-true checkbox and test the unchecked state end-to-end.
- Document the exact cap boundary (`1e21`) and range checks for precision/max_units.
