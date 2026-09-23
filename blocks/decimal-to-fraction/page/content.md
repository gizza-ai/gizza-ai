## About this tool

**Decimal to Fraction Calculator** converts a decimal into an exact fraction, a mixed number, or the simplest useful approximation. It is designed for measurement conversion, homework checks, recipe scaling, machining fractions, probability values, finance percentages and any place where `0.625` should become `5/8`.

The calculation uses exact integer arithmetic. Terminating decimals are written over powers of ten and reduced, repeating decimals are converted with the standard algebraic repeat formula, and approximations are chosen from continued-fraction convergents or the simplest fraction within a tolerance.

### Input forms it accepts

- Plain decimals: `0.625`, `-2.5`, `.125`.
- Grouped digits: `1,234.5`, `1_234.5`.
- Percentages: `12.5%` becomes `1/8`.
- Scientific notation: `1.25e-3` becomes `1/800`.
- Repeating decimals: `0.(3)`, `0.[3]`, `0.3...`, or set **Repeating trailing digits** to mark the last digits as repeating.

### Approximation controls

Leave **Tolerance**, **Maximum denominator** and **Fixed denominator** blank or zero for an exact fraction. Use **Maximum denominator** to get the best simple fraction under a cap, such as `3.14159265` with max denominator `100` → `311/99`. Use **Tolerance** to find the simplest fraction within an absolute error, such as `0.142857` within `0.000001` → `1/7`. Use **Fixed denominator** to snap to a measurement grid, for example nearest sixteenth.

**Rounding** controls whether approximations may go above or below the decimal. **Reduce to lowest terms** is on by default; turn it off when you intentionally want an unreduced fixed-denominator result such as `8/16`.

### Worked example

Input `0.625` has three decimal places, so it is first written as `625/1000`. Dividing numerator and denominator by their greatest common divisor `125` gives `5/8`, and the result is exact. The output also includes the mixed-number form, decimal error, percent error, continued-fraction convergents and a list of worked steps.

CLI example:

```bash
gizza tool decimal-to-fraction decimal=0.625
```

### Limits and edge cases

The parser accepts up to 18 digits before the decimal point, up to 18 fractional digits, repeating blocks up to 9 digits, and exponents from `e-18` to `e18`. Denominator caps and fixed denominators may be as large as 1,000,000,000,000. Tolerances must be less than 1 and either zero or at least `1e-12`. Inputs that would overflow exact arithmetic return a clear error instead of silently rounding.

## FAQ

<details>
<summary>How do I convert a decimal like 0.625 to a fraction?</summary>

Type `0.625` in the decimal field and leave the other controls blank. The tool writes it as `625/1000`, reduces it, and returns `5/8` exactly.

</details>

<details>
<summary>Can it handle repeating decimals?</summary>

Yes. Use notation such as `0.(3)` or `0.1(6)`, or enter a normal decimal and set **Repeating trailing digits** to the number of final digits that repeat. The repeating value is converted exactly before any approximation options are applied.

</details>

<details>
<summary>What is the difference between maximum denominator and fixed denominator?</summary>

**Maximum denominator** asks for the closest simple fraction whose denominator is at most the cap. **Fixed denominator** snaps to that exact denominator before optional reduction, which is useful for inches or other measurement grids such as sixteenths.

</details>

<details>
<summary>Why would I turn off reduction?</summary>

Reduction is usually best, but fixed-denominator workflows sometimes need the original grid. For example, `0.5` with fixed denominator `16` can stay as `8/16` when reduction is off, instead of reducing to `1/2`.

</details>

<details>
<summary>Does tolerance use relative or absolute error?</summary>

Tolerance is absolute decimal error. A tolerance of `0.000001` means the chosen fraction's decimal value may be at most one millionth away from the input.

</details>
