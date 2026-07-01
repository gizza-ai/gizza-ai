## About the geometry calculator

This free geometry calculator works out the key measures of a shape from its
dimensions. Choose a shape, fill in the dimensions that shape needs, and it
returns:

- **2D shapes** — the **area** and the **perimeter**.
- **3D shapes** — the **surface area** and the **volume**.

Everything runs locally in your browser. Nothing is uploaded to a server, and it
works offline once the page has loaded.

### Supported shapes

**2D (area + perimeter)**

- **Square** — `side`
- **Rectangle** — `width`, `height`
- **Triangle** — `base`, `height` (for the area); add `side_a`, `side_b`,
  `side_c` to also get the perimeter
- **Circle** — `radius`
- **Ellipse** — `radius_a` (semi-major), `radius_b` (semi-minor); perimeter uses
  Ramanujan's approximation
- **Trapezoid** — `base`, `top`, `height`; add `side_a`, `side_b` (the legs) for
  the perimeter
- **Parallelogram** — `base`, `side_a` (slant side), `height`
- **Regular polygon** — `sides` (how many), `side` (edge length)

**3D (surface area + volume)**

- **Cube** — `side`
- **Rectangular prism / box** — `width`, `height`, `length`
- **Sphere** — `radius`
- **Cylinder** — `radius`, `height`
- **Cone** — `radius`, `height`
- **Pyramid** (square base) — `base`, `height`

### Units

The calculator is unit-agnostic: enter every dimension in the same unit and the
results follow. Lengths come back in that unit, areas in unit² and volumes in
unit³ — the JSON marks each value's `unit` suffix (`""`, `²`, `³`).

### Examples

- **Circle**, `radius = 2` → area `12.566371`, perimeter `12.566371`
- **Rectangle**, `width = 3`, `height = 4` → area `12`, perimeter `14`
- **Sphere**, `radius = 3` → surface area `113.097336`, volume `113.097336`
- **Cylinder**, `radius = 2`, `height = 5` → volume `62.831853`

### How the values are computed

Standard formulas are used throughout — for example circle area is `π·r²`,
cylinder volume is `π·r²·h`, sphere volume is `(4/3)·π·r³`, and the regular
polygon area is `(n·s²) / (4·tan(π/n))`. Triangle side lengths are checked
against the triangle inequality before the perimeter is reported. Results are
rounded to six decimal places.

### FAQ

<details>
<summary>Which dimensions do I enter?</summary>

Only the ones the chosen shape uses — the rest
are ignored. The field labels list which shape each dimension belongs to.

</details>

<details>
<summary>Why is my triangle perimeter missing?</summary>

The perimeter needs all three sides
(`side_a`, `side_b`, `side_c`). With only `base` and `height` you still get the
area.

</details>

<details>
<summary>Is it free and private?</summary>

Yes. Your input never leaves your device, and it
works offline.

</details>
