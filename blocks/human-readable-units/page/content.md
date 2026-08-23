## About this tool

Human Readable Units converts raw magnitudes into the unit strings people expect in docs, dashboards and support tickets. Paste a byte count such as `1500000000`, a duration such as `123`, or a plain count such as `1200000`, then choose whether the value means data size, elapsed time or a large number.

Worked examples:

- `1500000000` with `kind=bytes` returns `1.5 GB`.
- `1073741824` with `kind=bytes` and `base=binary-iec` returns `1 GiB`.
- `11722000` with `kind=duration`, `input_unit=millisecond` and `max_units=3` returns `3h 15m 22s`.
- `1200000` with `kind=number` returns `1.2M`.

Use `base` to pick decimal bytes (`kB`, `MB`, `GB`), IEC binary bytes (`KiB`, `MiB`, `GiB`) or Windows-style binary labels (`KB`, `MB`, `GB`). Use `style` for short, long or narrow output, and turn on `detail=breakdown` when you need exact bytes, bits, clock time, ISO 8601 duration or scientific notation alongside the formatted value.

Limits and edge cases: values must parse as finite numbers after digit separators such as commas, spaces and underscores are removed. The converted magnitude is capped at `1e21` base units, `precision` is limited to `0` through `6`, and `max_units` is limited to `1` through `7`. Duration output truncates to the requested segment count instead of rounding up, so `3661` seconds with the default two segments is `1h 1m`.

## FAQ

<details>
<summary>Should I choose decimal or binary bytes?</summary>

Choose decimal for storage-device and network-style sizes where `1 kB = 1000 B`. Choose binary IEC when you need the unambiguous computer-memory scale where `1 KiB = 1024 B`. Binary JEDEC keeps the 1024 divisor but uses `KB`, `MB` and `GB` labels for compatibility with older tools and Windows-style displays.

</details>

<details>
<summary>Can I paste grouped numbers or scientific notation?</summary>

Yes. The parser accepts plain digits, commas, underscores, spaces and scientific notation. For example, `1,500,000,000`, `1_500_000_000` and `1.5e9` all represent the same value.

</details>

<details>
<summary>Why does a duration drop the last seconds?</summary>

The `max_units` setting controls how many non-zero segments are shown. With the default of two, `3661` seconds becomes `1h 1m`; set `max_units=3` to include `1s` as well. The tool truncates extra segments instead of rounding so timers and logs do not overstate elapsed time.

</details>

<details>
<summary>What does breakdown output include?</summary>

For byte counts, breakdown mode shows the primary value, exact bytes, decimal bytes, binary bytes and bits. For durations it adds exact seconds, short and long text, `HH:MM:SS` clock form and ISO 8601. For plain numbers it includes exact, short, long and scientific notation.

</details>
