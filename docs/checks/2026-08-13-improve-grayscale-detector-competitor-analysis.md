# Competitor analysis: grayscale-detector (2026-08-13)

## Search snapshot

Query: `online grayscale image detector check if image is grayscale RGB channels color pixels`

Representative tools reviewed from the results:

1. General grayscale converters that upload an image, preview a monochrome result, and expose a simple convert/download flow.
2. Technical image-check suites that include grayscale conversion or quality checks for image-prep workflows.
3. Pixel color picker / inspector tools that let users inspect individual pixel RGB/HSV values.
4. A direct PNG grayscale checker that accepts an uploaded image and reports whether it is grayscale.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor pattern | Gizza model fit | Decision for this tool |
| --- | --- | --- | --- |
| Upload or paste image input | Most tools use a file upload; some accept URL/paste flows. | Partially in-model. Current tool page can pass text fields reliably; CLI supports text params. | Use base64 or hex encoded bytes with multiline input. File upload is out-of-model for this pure block's current CLI contract. |
| Strict RGB grayscale check | Direct grayscale checkers compare whether pixels are effectively monochrome. | In-model. | Implement channel-delta metric: `max(R,G,B)-min(R,G,B)`. |
| Tolerance for compression noise | Converters/checkers often tolerate visually gray JPEG artifacts or allow settings. | In-model. | Add integer tolerance 0-255, default 2, with strict 0 and boundary 255 verified. |
| HSV/saturation-style color detection | Pixel inspectors expose HSV/HSL values that reveal tint beyond raw channel spread. | In-model. | Add `metric=channel_delta|saturation`; saturation catches dark tinted pixels. |
| Report image dimensions and counts | Quality-check tools summarize dimensions and pass/fail status. | In-model. | Report dimensions, scanned pixels, gray/color counts and percentages, max and mean score. |
| Sample offending pixels | Pixel pickers expose coordinates and RGB/hex values. | In-model. | Add `max_samples` 0-200 and list sample color pixel coordinates, hex, RGB, and score. |
| Alpha handling | Image tools often preview visible pixels; transparent colors may be hidden. | In-model. | `ignore_alpha=true` by default scores all RGB values; `false` skips fully transparent pixels and reports them. |
| JSON/report outputs | Developer-oriented tools expose machine-readable results or copyable text. | In-model. | Add `output=report|json`. |
| Convert image to grayscale and download | Grayscale converters transform and download a new image. | Out-of-model for this detector. | Do not transform images; document that this is an audit tool. |
| Batch image uploads / animation-wide analysis | Some image tools process multiple files or animated assets. | Out-of-model. | Single decoded image only; note animated formats are not audited frame-by-frame. |

## Descriptor impact

The descriptor includes every in-model table-stake parameter: `input`, `input_format`, `tolerance`, `metric`, `ignore_alpha`, `max_samples`, and `output`. The page mirrors these controls with enum labels, numeric placeholders, preset chips, and worked examples. Conversion/download and batch analysis are intentionally listed as limits rather than implied features.
