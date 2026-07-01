## About this tool

A fast, private unit converter that runs entirely in your browser — nothing is
uploaded to a server. Enter a value, the unit you have, and the unit you want,
and get the converted result instantly.

### Supported categories and units

- **Length** — metre, kilometre, centimetre, millimetre, micrometre, nanometre,
  mile, yard, foot, inch, nautical mile.
- **Mass** — kilogram, gram, milligram, microgram, tonne, pound, ounce, stone,
  US ton, UK (long) ton.
- **Temperature** — Celsius, Fahrenheit, Kelvin, Rankine, Réaumur.
- **Volume** — litre, millilitre, cubic metre, cubic centimetre, US/UK gallon,
  quart, pint, cup, fluid ounce, tablespoon, teaspoon.
- **Area** — square metre, square kilometre, square centimetre, square
  millimetre, hectare, acre, square mile, square yard, square foot, square inch.
- **Speed** — metre per second, kilometre per hour, mile per hour, foot per
  second, knot.
- **Data size** — bit, byte, and decimal (KB, MB, GB, TB, PB) plus binary
  (KiB, MiB, GiB, TiB) multiples.
- **Time** — nanosecond, microsecond, millisecond, second, minute, hour, day,
  week, month, year.

### How to use

1. Type the number you want to convert.
2. Enter the unit you are starting from (e.g. `km`, `lb`, `celsius`, `gallon`).
3. Enter the unit you want to convert to (in the **same** category).

Unit names, plurals, and common symbols/aliases all work — for example `m`,
`metre`, and `meters` are interchangeable, as are `c`/`celsius` and `mph`/`mile
per hour`. The "from" and "to" units must belong to the same category; you can
convert metres to feet, but not metres to kilograms.

### Notes on accuracy

Conversions use exact internationally-defined factors where they exist (for
example 1 inch = 2.54 cm exactly, 1 pound = 0.453 592 37 kg exactly). Months use
the average Gregorian month length and years use the Julian year (365.25 days).
Data sizes distinguish decimal SI multiples (KB = 1000 bytes) from binary IEC
multiples (KiB = 1024 bytes).

## FAQ

<details>
<summary>Do I have to select a category before converting?</summary>

No — the category is inferred from the units you type. If the two units don't
belong together you get a specific error rather than a wrong answer, e.g.
converting `metre` to `kilogram` reports "cannot convert metre (length) to
kilogram (mass)".

</details>

<details>
<summary>Is a gigabyte (GB) the same as a gibibyte (GiB)?</summary>

No, and the tool keeps them separate: `kb`/`mb`/`gb`/`tb`/`pb` are decimal SI
multiples (1 GB = 1000 MB = 10⁹ bytes) while `kib`/`mib`/`gib`/`tib` are binary
IEC multiples (1 GiB = 1024 MiB = 2³⁰ bytes). You can convert between the two
systems directly — 1 GiB ≈ 1.074 GB. Bits are supported too (8 bits = 1 byte).

</details>

<details>
<summary>Are "gallon", "pint" and "cup" US or imperial measures?</summary>

The bare names default to US measures: `gallon`, `quart`, `pint`, `cup` and
`fluid-ounce` are all US units. For the imperial versions use `uk-gallon` or
`imperial-gallon`. Likewise `ton` is ambiguous, so use `us-ton`/`short-ton` or
`long-ton`/`imperial-ton` explicitly.

</details>

<details>
<summary>How many digits of precision does the result keep?</summary>

Results are rounded to 12 significant figures with trailing zeros trimmed, and
extreme magnitudes (10¹⁵ and up, or below 10⁻⁹) switch to scientific notation.
The underlying factors are the exact defined values where one exists, so
round-trips like metres → feet → metres come back to the number you started
with.

</details>
