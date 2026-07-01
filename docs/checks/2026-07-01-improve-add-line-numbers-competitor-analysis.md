# add-line-numbers competitor analysis (2026-07-01)

## Scope

Tool: `add-line-numbers`

Goal: prefix every line of pasted text with sequential line numbers, locally in the browser and through the gizza CLI/chat surfaces.

## Competitors reviewed

1. Online Text Tools — Add Line Numbers to Text
   - Simple browser utility: paste text and receive numbered lines.
   - Gap to close: immediate paste-in workflow and plain output suitable for copying.
   - Shipped: browser-local page with multiline input and text output.

2. Online Text Tools — Add Prefix to Text Lines
   - Related prefixing tool shows users often need configurable text before each line.
   - Gap to close: separator customization, not just hard-coded `1.` output.
   - Shipped: configurable separator string between number and line content.

3. EasyToolWeb — Online Text Numbering Tool
   - Describes line numbers, prefixes, and suffixes for documents, code, and notes.
   - Gap to close: support non-default starting values and increments for reference lists.
   - Shipped: `start` and `step` parameters.

4. i2Text — Add Line Numbers
   - Offers alternative numbering styles and use cases around reviewing/editing.
   - In-model gap: numbered output should be easy to align and scan.
   - Shipped: alignment options for no padding, right-aligned spaces, and zero padding.
   - Not built: letters/Roman numerals; this tool stays numeric to match the backlog description.

5. Gillmeister Software — Number lines online
   - Offers arbitrary starting value and leading-zero numbering.
   - Gap to close: leading zero support and skip-blank-line behavior familiar from command-line tools.
   - Shipped: zero padding plus `number_nonblank` mode similar to `cat -b`.

## In-model improvements shipped

- Line numbering with configurable start, step, separator, and padding.
- `number_nonblank` mode that leaves blank/whitespace-only lines unnumbered and does not consume numbers.
- Descriptor drift guard, unit tests, wafer fixture, web wrapper, generated page, CLI smoke, and Playwright coverage.
- Original page copy documenting `nl`/`cat -n` style behavior and browser-local privacy.

## Out-of-model / not built

- Alphabetic or Roman numeral counters.
- Suffix-only or arbitrary prefix/suffix templates beyond the separator.
- File upload, download buttons, batch processing, or saved presets.

## Verification notes

The final verification matrix includes block cargo tests, wafer build, wasm-pack web build, generator, CLI smoke, and Playwright page tests for defaults, custom start/step/separator/padding, and skip-blank-line behavior.
