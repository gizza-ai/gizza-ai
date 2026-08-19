## About this tool

Polar coordinates describe a point by how far it is from the origin (`r`) and which way it lies (`θ`), instead of by how far along each axis it sits. The conversion itself is two lines of maths — `r = √(x² + y²)` and `θ = atan2(y, x)` — but doing it by hand for a few hundred survey points, antenna bearings, or scatter-plot samples is tedious and easy to get wrong in the second and third quadrants.

This converter runs the maths over a whole CSV at once, locally in your browser. It reads your header row, finds the coordinate columns (by name, by 1-based number, or by auto-detecting the usual spellings such as `x`/`y`, `easting`/`northing`, `r`/`rho`, `theta`/`phi`), converts every data row, and carries the columns it did not touch straight through to the output. Set the direction to **Polar → Cartesian** to go back the other way with `x = r·cos θ` and `y = r·sin θ`.

**Worked example.** Paste this:

```
id,x,y
p1,3,4
p2,-3,-4
p3,-1,0
```

with the angle unit set to degrees, the range set to signed, and 2 decimal places. The result is:

```
id,r,theta
p1,5.00,53.13
p2,5.00,-126.87
p3,1.00,180.00
```

`p1` and `p2` are the same distance from the origin but sit in opposite quadrants, and the angles differ by 180° — that is `atan2` doing its job. Switch the range to *positive* and `p2`'s angle becomes `233.13` instead.

**Angle units and ranges.** Degrees (a full turn is 360), radians (2π), gradians (400) and turns (1) are all available, and the same setting is used to *read* the angle when you convert polar → Cartesian. The signed range is `atan2`'s natural `(−180°, 180°]`; the positive range wraps negative angles up into `[0°, 360°)`. Both scale with whichever unit you pick.

**Limits and edge cases.** Input is capped at 5 MB and 200,000 data rows. Numbers are 64-bit floats, so about 15–17 significant digits are meaningful and the decimal-places control tops out at 15. The origin `(0, 0)` returns `r = 0` and `θ = 0`, since no direction is defined there. A cell that is empty or is not a number stops the run and names the offending row and column rather than quietly emitting a blank. Delimiters are sniffed from the first non-empty line (comma, semicolon, tab or pipe) and CSV output is written back with the same one. This tool is planar only — it does not do 3D, cylindrical, spherical, or geodetic (lat/lon) coordinate systems, and it does not draw plots.

## FAQ

<details>
<summary>Why is my angle negative?</summary>

The default range is the signed one, `(−180°, 180°]`, which is what `atan2` returns and what most maths libraries use. Points below the x-axis therefore get a negative angle: `(3, −4)` is `−53.13°`. If you would rather see compass-style values from 0 up to a full turn, set the angle range to *positive* and that same point becomes `306.87°`.

</details>

<details>
<summary>Does it handle all four quadrants correctly?</summary>

Yes. The conversion uses `atan2(y, x)`, not `arctan(y / x)`. Plain `arctan` only produces angles between −90° and 90°, so it collapses `(3, 4)` and `(−3, −4)` onto the same answer and blows up when `x` is 0. `atan2` looks at the signs of both coordinates, so `(3, 4)` gives `53.13°`, `(−3, −4)` gives `−126.87°`, and `(0, 5)` gives exactly `90°`.

</details>

<details>
<summary>My CSV has id and label columns — will they survive?</summary>

Yes, as long as *Keep other columns* stays on. Every column that is not one of the two coordinate columns is copied through in its original order, and the two converted values are appended after them. So `id,x,y` becomes `id,r,theta`. Turn the option off if you want just the converted pair and nothing else.

</details>

<details>
<summary>How do I tell it which columns to use?</summary>

Leave both column fields empty and it auto-detects common header names — `x`, `y`, `easting`, `northing` for Cartesian input, and `r`, `rho`, `radius`, `theta`, `phi`, `angle` for polar input. If your headers are unusual, type the exact header name (matching is case-insensitive) or a 1-based column number, such as `2` and `3`. With *First row is a header* turned off, the columns are named `column1`, `column2`, … and the first two are used by default.

</details>

<details>
<summary>Can it convert polar coordinates back to x and y?</summary>

Yes — set the direction to **Polar (r, θ) → Cartesian (x, y)**. It reads the radius and angle columns, interprets the angle in whichever unit you selected, and emits `x = r·cos θ` and `y = r·sin θ`. Round-tripping `(3, 4)` through polar and back returns `3` and `4` again, subject to the decimal places you asked for.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion is a small WebAssembly module that runs entirely in your browser tab. The CSV you paste is never sent to a server, and the same code is what the command-line version runs locally.

</details>
