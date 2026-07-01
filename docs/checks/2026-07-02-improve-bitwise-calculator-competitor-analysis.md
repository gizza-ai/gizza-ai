# Competitor analysis: bitwise-calculator

Date: 2026-07-02
Tool: `bitwise-calculator`

Scan done BEFORE implementation (one WebSearch, top-3 skim) to set table stakes.

## Competitors reviewed

| competitor | useful capabilities observed | gaps considered |
| --- | --- | --- |
| Omni Calculator (bitwise) | AND/OR/XOR on binary/octal/decimal input; selectable bit width (4/8/12/16, default 8); shows result in all bases; shows both signed and unsigned decimal readings; rejects operands wider than the chosen width. | No NOT, shifts, rotates, or popcount — our backlog row requires them. Width choices are electronics-flavored (4/12); programmer-standard 8/16/32/64 fits gizza's audience better. |
| ToolSlick XOR calculator | XOR on binary/octal/decimal/hex/ASCII with base auto-detection; result rendered in every base simultaneously; multi-operand chains with delimiters; intermediate-step table. | Multi-operand chains and ASCII-text XOR are a cipher-style stream feature (out of a single two-operand model); base handled via 0b/0o/0x prefixes instead of a separate selector; intermediate-step display deferred. |
| CircuitDigest bitwise calculator | AND/OR/XOR (NOT in the UI) on binary/decimal/hex input; results in binary/octal/decimal/hex; worked truth-table style examples in the copy. | No bit-width control, no shifts/rotates/popcount; examples pattern adopted (as one-click chips + a worked example in the page copy), copy not reused. |

## Table stakes derived (all in-model, all implemented)

- Two operands + an operation selector; result shown in binary, octal, decimal, hex simultaneously.
- Input in binary/octal/decimal/hex — via `0b`/`0o`/`0x` prefixes (plain digits = decimal), with `_`/space digit separators, instead of a separate base dropdown.
- Selectable bit width (8/16/32/64, default 32) that masks results and bounds operands, with a clear out-of-range error.
- Both unsigned and signed (two's-complement) decimal readings of the result, Omni-style.
- Negative decimal input (e.g. `-8`) interpreted as two's complement at the chosen width.
- Worked examples on the page + one-click example chips.

## Differentiators beyond the top-3 (from the backlog description, in-model)

- Left/right logical shifts (`shl`, `shr`), left/right rotates (`rotl`, `rotr`), NOT, and popcount — none of the three reviewed tools offer shifts/rotates/popcount.
- Shift counts ≥ width yield 0 (logical shift semantics); rotate counts wrap modulo the width.

## Out-of-model or deferred gaps

- Multi-operand chains (`a XOR b XOR c …`) with delimiters — the descriptor models one operation on ≤2 operands; chains are a pipe/recipe feature.
- ASCII/text XOR of strings — stream-cipher territory (gizza has cipher tools); this tool is integer-only.
- Intermediate calculation table (ToolSlick) — single-step tool; the binary rendering already shows the aligned result.
- Arithmetic (sign-filling) right shift — `shr` is documented as logical/zero-fill; an `asr` op can be added later if requested.

Original analysis only; no competitor copy, branding, or assets were copied.

Sources: [Omni Calculator](https://www.omnicalculator.com/math/bitwise), [ToolSlick](https://toolslick.com/math/bitwise/xor-calculator), [CircuitDigest](https://circuitdigest.com/calculators/bitwise-calculator)
