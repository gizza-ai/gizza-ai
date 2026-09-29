## Turn a spreadsheet column into scannable barcodes

Paste a list of SKUs, asset tags or retail codes — one per line, or as CSV/TSV rows — and get
every barcode back at once. The whole batch is generated in your browser by a WebAssembly
encoder: nothing is uploaded, nothing is stored, and the same input always produces the same
ZIP bytes, so a batch can be regenerated in a build pipeline and diffed.

Two bundles are available:

- **ZIP** — one PNG and/or SVG per row, plus an `index.csv` manifest that maps each filename
  back to the value, the symbology used and whether the row succeeded.
- **Printable PDF label sheet** — every barcode laid out on an Avery-style grid, drawn as
  vectors so it prints at the printer's own resolution. Each label auto-fits: the bar width
  shrinks until the symbol and its quiet zones fit the label, so the same list prints correctly
  on a 65-up address sheet and a 10-up shipping sheet without re-tuning anything.

### Worked example — three SKUs to a Code 128 ZIP

Symbology `code128`, input format `csv`, column mapping `value-name`, files `svg`:

```
SKU-1001,widget
SKU-1002,gadget
SKU-1003,gizmo
```

The ZIP contains `widget.svg`, `gadget.svg`, `gizmo.svg` and `index.csv`. The manifest reads:

```
filename,value,symbology,status
widget.svg,SKU-1001,Code 128,ok
gadget.svg,SKU-1002,Code 128,ok
gizmo.svg,SKU-1003,Code 128,ok
```

Each SVG is a `230`-ish pixel wide symbol with the value printed underneath. Because the values
mix letters and digits, Code 128 uses subset B; a digit-only value such as `12345678` is
automatically encoded in the double-density subset C instead, which is about 40% narrower.

### Worked example — UPC-A with the check digit calculated

Retail spreadsheets usually hold the 11-digit product code without its trailing check digit.
Set symbology `upca`, leave **Calculate missing check digits** on, and paste:

```
03600029145
```

The barcode encodes — and prints under the bars — `036000291452`. The `2` is the GS1 mod-10
check digit, computed from the 11 digits you pasted. Paste the full 12 digits instead and the
check digit is *verified*: a wrong one is reported as `check digit 3 is wrong for 03600029145 —
expected 2` rather than silently encoded into an unscannable label.

The same rule covers EAN-13 (12 → 13 digits), EAN-8 (7 → 8) and ITF-14 (13 → 14).

### Symbologies

| Symbology | Accepts | Typical use |
|---|---|---|
| Code 128 | any printable ASCII | SKUs, asset tags, serials — the general-purpose default |
| Code 39 | `0-9 A-Z` space `- . $ / + %` | older warehouse and automotive labels |
| Code 93 | same as Code 39, more compact | where Code 39 is too wide |
| EAN-13 | 13 digits (or 12 + auto check) | retail products outside North America |
| EAN-8 | 8 digits (or 7 + auto check) | small retail packaging |
| UPC-A | 12 digits (or 11 + auto check) | retail products in North America |
| ITF / ITF-14 | even digit count (odd + auto check) | shipping cartons, outer cases |
| Codabar | `0-9 - $ : / . +` | libraries, blood banks, photo labs |

Set symbology to **Auto-detect per row** to pick by shape instead: 14 digits become ITF-14,
13 become EAN-13, 12 become UPC-A, 8 become EAN-8, and everything else becomes Code 128.

### Limits and edge cases

- **500 rows** per batch and **120 characters** per value. Split a longer list and run it twice —
  the cap is a memory guard for the in-browser sandbox, not a paywall.
- **32 MB** of generated images per ZIP. Lower the bar height or bar width if you hit it.
- **Bad rows do not kill the batch.** A row that cannot be encoded is skipped, counted in the
  result line, and written into `index.csv` with its line number and the reason. Only a batch
  where *no* row encodes is an error.
- **The quiet zone matters.** Code 128 and ITF need at least 10 blank modules either side to
  scan reliably, which is the default. Lowering it produces a smaller image that many scanners
  will refuse to read.
- **Bars cannot be transparent**, and a light-bars-on-dark-background combination will not scan
  on most hardware. The background *can* be `transparent` (useful for SVG overlays).
- **Human-readable text is the encoded value**, including any check digit that was calculated —
  not an arbitrary caption. The second CSV column is the output *filename*, not the caption.
- **Bars render at a uniform height with the text centred below.** EAN/UPC guard-bar extensions
  and the split digit groups you see on retail packaging are not drawn; the symbol itself is
  fully spec-conformant and scans normally.
- **PNG text uses an embedded 8×8 bitmap font** (so no font file ships and no network is
  touched), which is why the text size snaps to whole multiples of 8 pixels. SVG uses the
  viewer's monospace font. Alternate typefaces and above-the-symbol placement are not offered.
- **Duplicate filenames are de-duplicated**, not overwritten: a second `same.svg` becomes
  `same-2.svg`.
- **Print the PDF at 100% scale** with page scaling off. "Fit to page" shrinks the sheet by a
  few percent, which is enough to push the last column off its labels.

## FAQ

<details>
<summary>Which barcode type should I use for my SKUs?</summary>

Use **Code 128** unless something external dictates otherwise. It encodes letters, digits and
punctuation, it is the most compact symbology for mixed data, and every modern scanner reads it.
Choose **EAN-13** or **UPC-A** only when the codes are real retail product numbers issued
through GS1 — those symbologies accept digits only and enforce a check digit. Use **ITF-14**
for outer shipping cartons, and **Code 39** only when you are matching a legacy system that
already requires it.

</details>

<details>
<summary>My 12-digit codes are rejected as EAN-13. Why?</summary>

EAN-13 is 13 digits: 12 of data plus a check digit. With **Calculate missing check digits** on,
pasting 12 digits is fine — the 13th is computed for you. If you are seeing a rejection, the
values are probably 12-digit **UPC-A** codes that already include their check digit, so switch
the symbology to `upca`. Symbology `auto` makes this decision for you: it reads a 12-digit value
as UPC-A and a 13-digit value as EAN-13, which is the convention retail spreadsheets use.

</details>

<details>
<summary>How do I control the filename of each barcode?</summary>

Add a second column. With the default `auto` (or explicit `value-name`) column mapping, a row
like `SKU-1001,widget` encodes `SKU-1001` and writes `widget.png`. If your spreadsheet puts the
name first, choose `name-value` instead. Rows with no name column get numbered automatically
from the **Auto filename prefix** — `barcode-001.png`, `barcode-002.png`, and so on. If your
values legitimately contain commas, choose `value-only` so the whole line is treated as the
barcode payload.

</details>

<details>
<summary>Will the printable sheet line up with my label stock?</summary>

The six presets use the published grid geometry for those stock sizes — page size, first-label
offset, label size and the pitch between labels — so the layout is correct as long as your
printer does not rescale it. Print at **100% / actual size** with "fit to page" and any
borderless mode switched off, then check one sheet against the stock before running the rest.
If your stock is not listed, the generic A4 grid (40 up, 45 × 25 mm) is a common cut-to-fit
size, or use the ZIP output and place the images yourself.

</details>

<details>
<summary>What happens to rows that cannot be encoded?</summary>

They are reported, never silently dropped. The result line tells you how many rows failed and
shows the first few reasons, and `index.csv` carries one row per failure with the original line
number — for example `error (line 4): value must contain digits only`. The rest of the batch
still generates. Only if *every* row fails do you get an error instead of a download, and that
error quotes the first failure so you can see what went wrong.

</details>

<details>
<summary>Does anything I paste get uploaded?</summary>

No. The encoder, the PNG/SVG renderers and the PDF writer are all compiled to WebAssembly and
run inside this page. There is no upload, no server round-trip and no analytics on the values
you paste. You can confirm it by loading the page, disconnecting from the network, and
generating a batch — it still works.

</details>
