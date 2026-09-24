## About this tool

Use this scientific calculator when a basic four-function calculator is not enough. It supports common scientific functions (`sin`, `cos`, `tan`, `log`, `ln`, `sqrt`, `ncr`, `gcd`, and more), constants (`pi`, `e`, `tau`, `phi`, `i`), implicit multiplication (`2pi`, `3(4+5)`), complex arithmetic, named variables, and worksheet-style multi-line evaluation.

Worked examples:

- `sin(90) + cos(0)` with angle unit set to degrees returns `2`.
- `sqrt(-1)` returns `i`.
- A worksheet such as `x = 7`, `y = x^2`, `y + ans` evaluates top-to-bottom and returns each line's result.

The calculation engine is deterministic and runs locally in your browser. Display precision can be set up to 200 digits for consistent formatting across surfaces, but the current engine uses double-precision arithmetic internally; use the higher display values for formatting, not for exact arbitrary-precision proofs.

## Limits and edge cases

- Complex numbers use `i`, for example `2 + 3i` or `sqrt(-1)`.
- Trig functions honor the selected angle unit. Hyperbolic functions always use their usual unitless definitions.
- Factorial is limited to integers from 0 through 170, the safe finite range for double precision.
- Functions that are defined only for real numbers, such as `gcd` or `floor`, return a clear error if passed a complex value.
- `%` means modulo, not percent.

## FAQ

<details>
<summary>How do I define and reuse variables?</summary>

Put assignments either in the Variables box or directly in the Expression box. Lines are evaluated from top to bottom, so `x = 7`, then `y = x^2`, then `y + ans` reuses both `x` and the previous result.

</details>

<details>
<summary>Can this calculator handle complex numbers?</summary>

Yes. Use `i` as the imaginary unit: `sqrt(-1)` returns `i`, `2+3i` works through implicit multiplication, and helper functions such as `re`, `im`, `arg`, and `conj` are available.

</details>

<details>
<summary>Does the precision setting make calculations arbitrary precision?</summary>

No. The setting controls displayed significant digits and output formatting. The engine currently uses double-precision arithmetic internally, so the page documents rounding and domain limits instead of pretending to be an exact symbolic or arbitrary-precision CAS.

</details>

<details>
<summary>Why does a value like sin(pi) show a tiny non-zero number?</summary>

Floating-point constants and trig functions are approximate. Values such as `sin(pi)` may show a very small residue instead of exactly zero; use `round(...)` when you need a cleaned display.

</details>
