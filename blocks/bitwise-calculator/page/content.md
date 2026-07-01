## About this tool

The bitwise calculator applies one bitwise operation to integers at a fixed bit
width and shows the result in every common base at once. It covers the
two-operand operations **AND**, **OR** and **XOR**, the one-operand operations
**NOT** and **popcount** (count of set bits), plus logical shifts (**shl**,
**shr**) and rotates (**rotl**, **rotr**). Everything runs locally in your
browser — nothing is sent to a server.

Operands can be written in any base: plain digits are decimal, `0x57` is hex,
`0b0101_0111` is binary, `0o127` is octal. Underscores or spaces may separate
digits, and a leading `-` (e.g. `-8`) is read as two's complement at the chosen
bit width. The result is rendered in binary (nibble-grouped), octal, decimal,
hex and as a signed two's-complement value.

### Worked example

`87 AND 101` at 8 bits — align the binary to see which bits survive:

```text
  0101 0111   (87, 0x57)
& 0110 0101   (101, 0x65)
= 0100 0101   (69, 0x45)
```

The tool prints that result in every base:

```text
operation: 87 AND 101 (8-bit)
binary   : 0100 0101
octal    : 0o105
decimal  : 69
hex      : 0x45
signed   : 69
```

### Limits

- Bit widths: 8, 16, 32 or 64 (default 32). Operands must fit the chosen width —
  at 8 bits that is 0..255 unsigned or -128..127 signed; out-of-range input is
  reported as an error, never silently truncated.
- `shr` is a **logical** (zero-fill) right shift; an arithmetic (sign-fill)
  shift is not offered.
- Shifting by the width or more yields 0; rotate counts wrap modulo the width.
- One operation per run — a chain like `a XOR b XOR c` takes two runs.

## FAQ

<details>
<summary>How are negative numbers handled?</summary>

A leading `-` is interpreted as two's complement at the selected bit width, so
`-8` at 8 bits is the bit pattern `1111 1000` (240 unsigned). Results with the
top bit set are shown both ways: the `decimal` line is the unsigned reading and
the `signed` line is the two's-complement reading. Inputs below the signed
minimum for the width (e.g. `-129` at 8 bits) are rejected.

</details>

<details>
<summary>Is the right shift logical or arithmetic?</summary>

`shr` is a **logical** shift: vacated high bits are filled with 0, regardless of
the sign bit. That matches `>>` on unsigned integers in C/Rust/Go and `>>>` in
JavaScript/Java. An arithmetic (sign-propagating) right shift is not offered —
if you need one on a negative value, divide the signed reading by a power of two
instead.

</details>

<details>
<summary>What happens if I shift or rotate by more than the bit width?</summary>

Shifts are taken literally: shifting an 8-bit value by 8 or more pushes every
bit off the edge, so the result is 0. Rotates wrap around instead — the count is
reduced modulo the width, so `rotl` by 9 on an 8-bit value is the same as `rotl`
by 1, and rotating by exactly the width returns the value unchanged.

</details>

<details>
<summary>Which input formats does the calculator accept?</summary>

Decimal (plain digits, e.g. `87`), hex with `0x` (e.g. `0x57`), binary with `0b`
(e.g. `0b0101_0111`) and octal with `0o` (e.g. `0o127`). Underscores and spaces
between digits are ignored, so you can group long values for readability. The
shift/rotate count in the second field accepts the same formats but must be
non-negative.

</details>

<details>
<summary>Why does NOT return a large positive number?</summary>

NOT inverts every bit inside the selected width, and the `decimal` line always
shows the unsigned reading of that pattern. `NOT 0x0F` at 8 bits is
`1111 0000` = 240 unsigned — read the `signed` line (-16) for the
two's-complement interpretation. Pick a larger or smaller bit width to control
how many high bits get inverted.

</details>
