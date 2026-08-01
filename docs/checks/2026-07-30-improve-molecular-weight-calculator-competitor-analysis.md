# molecular-weight-calculator competitor analysis — 2026-07-30

## Sources skimmed

- Chemistry utility pages for molecular weight / molar mass from a formula.
- General chemistry calculators that include formula examples and percent composition.
- Formula-focused calculators that expose precision/significant-digit controls and example presets.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitor tools | Model fit | Decision |
| --- | --- | --- | --- |
| Chemical formula input (`H2O`, `NaCl`, `C6H12O6`) | Universal | In-model | Required `formula` string parameter and page field. |
| Common example presets | Common | In-model | Added chips for water, glucose, caffeine, calcium hydroxide, and copper sulfate hydrate. |
| Average molar mass / molecular weight | Universal | In-model | Reported as `molar_mass` in `g/mol`. |
| Exact / monoisotopic mass | Common in richer tools | In-model | Reported as `monoisotopic_mass` in Da using most-abundant-isotope masses. |
| Element counts and distinct-element count | Common | In-model | Reported as `atom_count`, `element_count`, and Hill notation. |
| Elemental composition and percent by mass | Common | In-model | Returned as per-element JSON rows with atomic weight, mass contribution, and percent. |
| Precision / significant digits control | Some tools | In-model | Added `decimals` parameter, 0–10, default 4, with a page slider. |
| Parentheses / grouped formulas | Expected for chemistry formulas | In-model | Implemented nested `()`, `[]`, and `{}` groups. |
| Hydrates / dot notation | Common chemistry need | In-model | Implemented middle-dot, period, bullet, and asterisk separators with segment coefficients. |
| SMILES input | Offered by some richer tools | Out-of-model for this block | Not implemented; full SMILES graph parsing is beyond the current pure formula model. The tool errors clearly and documents formula-only input. |
| Isotope distribution / isotope-pattern chart | Specialist chemistry feature | Out-of-model | Not built; exact mass is a single most-abundant-isotope sum, not a spectrum. |
| Structure drawing / molecule editor | Specialist chemistry UI | Out-of-model | Not built; this repo's generated page model fits text inputs, not a chemistry drawing canvas. |

## Implementation notes

The final design is a pure Rust formula parser with an embedded element table. It favors deterministic, local calculation over external chemistry services: no network lookup, no model dependency, and no SMILES conversion. The page copy calls out unsupported SMILES and isotope-pattern features so competitor capabilities are not silently omitted.
