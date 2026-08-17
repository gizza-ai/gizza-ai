# raw-rgb-to-png competitor analysis (2026-08-17)

Tool: `raw-rgb-to-png` — assemble headerless RGB/RGBA pixel bytes plus explicit width and height into a PNG image.

Research method: reviewed common "raw RGB to image/PNG" utilities and the usual developer workflows: ImageMagick `convert -size WxH -depth 8 rgb:input.rgb out.png`, ffmpeg rawvideo commands, and browser snippets that paste hex/base64 bytes into a canvas. Observations are paraphrased; no competitor copy or branding was reused.

## Sources scanned

1. **ImageMagick raw RGB import workflows.** The table-stakes inputs are width, height, bit depth (usually 8-bit), channel order/pixel layout, and the raw byte stream. These tools are powerful but command-line oriented and assume a local file.
2. **ffmpeg rawvideo examples.** The common knobs are `-pixel_format rgb24|rgba`, `-video_size WxH`, and a raw input file. They are frame/video capable, but for a single still image the core requirement is the same: dimensions + pixel format + bytes.
3. **Browser/canvas raw byte demos.** These usually accept arrays, hex strings, or base64 strings, draw into ImageData, and let the user save a PNG. They rarely validate byte counts clearly.

## Table-stakes and decisions

| Capability / UX pattern | Seen elsewhere | In gizza model? | Decision |
| --- | --- | --- | --- |
| Explicit width and height | All tools | Yes | Required integer params with 1-8192 axis limits and a 16 MP pixel cap. |
| RGB and RGBA layouts | All tools | Yes | `pixel_format=rgb|rgba`; rgb = 3 bytes/pixel, rgba = 4 bytes/pixel. |
| Hex input | Browser snippets/debug workflows | Yes | Default `encoding=hex`; accepts grouped byte pairs and `0x` tokens. |
| Base64 input | Browser snippets/data URLs | Yes | `encoding=base64`; accepts data URL prefixes and optional padding. |
| Decimal byte arrays | Canvas snippets | Yes | `encoding=decimal`; accepts 0-255 values separated by commas/spaces. |
| Row padding / stride | Framebuffer dumps | Yes | Optional `row_stride` drops padding bytes per row. |
| Exact byte-count validation | Often weak | Yes | Errors state expected bytes, shortfall/excess, dimensions and format. |
| PNG output | All tools | Yes | Returns an `image/png` media envelope named `raw-rgb.png`. |
| Infer dimensions automatically | Some viewers try heuristics | Out-of-model/rejected | Ambiguous without a header; explicit dimensions are safer. |
| BGR/BGRA, grayscale, 16-bit, planar YUV | ImageMagick/ffmpeg | Out-of-model for v1 | These are separate pixel formats with different conversion rules; add later as descriptor enum variants if needed. |
| Multi-frame rawvideo | ffmpeg | Out-of-model | gizza's current block surface returns one output artifact; this tool is intentionally one still image. |
| Standalone page preview | Browser demos | Out-of-model for this block | The output is binary image bytes from a no-input-file pure block; existing gizza page generator does not render this no-page media-envelope shape. CLI/chat surfaces are the honest fit. |

## Defaults chosen

- `encoding=hex` because developer byte dumps most often arrive as hex and it is readable in examples.
- `pixel_format=rgb` because raw RGB24 is the most common headerless still-image layout and avoids alpha surprises.
- `row_stride=0` means tightly packed rows; users with padded framebuffer rows can opt in explicitly.

## Outcome

The in-model scope is implemented: headerless 8-bit RGB/RGBA bytes to one PNG, with hex/base64/decimal input and optional stride. Larger raw-video, inferred-size and expanded pixel-format support are documented as out of scope rather than hidden behind guesses.
