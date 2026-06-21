# qr-code-generator — competitor analysis (2026-06-21)

New tool built from the backlog and improved against the top free online QR-code
generators. Generates a QR code as **SVG or PNG** from 8 content types, runs
fully locally (pure-Rust `qrcode` + `image`), and the encoded payload never
leaves the device. Surfaces: chat (LLM) + CLI. No page surface — image-bytes
output has no page render mode (same as `wifi-qr-code-generator` and the chart
tools).

## Surfaces verified

- **Chat / LLM API:** `wafer build` validates the `block.wasm` instantiates
  (529.9 KiB). Drift-guard test (`schema_json_matches_authored_chat_schema`)
  passes — the descriptor, manifest.json and authored schema agree.
- **CLI:** `gizza tool qr-code-generator …` exercised for every content type and
  both formats (see matrix below). Output written to `qr-code.svg` / `.png`.
- **Roundtrip scannability:** generated PNGs were decoded back with the existing
  `qr-decode` core (`quircs`) — text, url, wifi and a coloured High-ECC code all
  decoded to exactly the encoded payload, proving the codes are scannable.

## Competitors surveyed

| Tool | Content types | Formats | Customisation |
|------|---------------|---------|---------------|
| qr-code-generator.com | URL, vCard, text, email, SMS, WiFi, Twitter, Bitcoin | PNG/SVG/EPS (signup to download) | colours, logo (paid/dynamic) |
| QRCode Monkey | URL, text, email, phone, SMS, vCard, WiFi, location, … | PNG/SVG/PDF/EPS | colours, logo, design |
| goQR.me | URL, text, email, SMS, phone, vCard, MeCard, WiFi, geo, calendar event | PNG/JPEG/GIF/SVG/EPS/PDF | fg/bg colour, border, error-correction L/M/Q/H |
| GenQRCode | text, URL, vCard, … | SVG/EPS/TIFF/PNG/GIF/WEBP/JPEG + 3D | colours, image |
| Adobe Express | URL, text | PNG/JPEG/PDF | colours, logo |

Sources: qr-code-generator.com, qrcode-monkey.com, goqr.me, genqrcode.com,
adobe.com/express, forqrcode.com (web search + page fetch, 2026-06-21).

## Gaps closed (in-model)

- **More content types.** Started at text/url/wifi/contact; added **email**
  (`mailto:` with url-encoded subject/body), **sms** (`SMSTO:number:message`),
  **phone** (`tel:`), and **geo** (`geo:lat,lng`, validated numeric) — matching
  the common set every major competitor offers.
- **PNG output, not just SVG.** Competitors all offer a raster download; we
  render PNG via the `image` crate (`size` param, 64–2048 px, clamped) in
  addition to scalable SVG.
- **Error-correction control.** `error_correction` = L/M/Q/H (default M), exactly
  the goQR.me/QRCode-Monkey control.
- **Custom colours.** `dark_color` / `light_color` (`#rgb` or `#rrggbb`,
  validated) recolour the modules and background, applied to both SVG and PNG.
- **URL convenience.** `content=url` prepends `https://` to a bare host so the
  code opens as a link (mirrors competitor URL handling).

## Out-of-model (intentionally not built)

- **Logo / image embedding in the centre of the code.** A design/compositing
  feature; competitors gate it behind paid/dynamic plans. Out of scope for a
  single-shot pure-compute tool.
- **Dynamic QR codes / tracking / analytics.** These require a hosted redirect
  service and a backend; gizza tools are browser-local and stateless.
- **Bulk / campaign generation, 3D (STL/OBJ) and exotic vector formats
  (EPS/TIFF).** Niche; SVG (vector) + PNG (raster) cover the practical needs and
  SVG re-exports trivially to EPS/PDF in any vector editor.
- **Calendar-event (iCal) payload.** Reasonable future addition, but rarer than
  the 8 types shipped; deferred to keep the parameter surface focused.

## NEVER did

No competitor copy, branding, trademarks, logos or UI was copied. All payload
formats used (`WIFI:`, vCard 3.0, `mailto:`, `SMSTO:`, `tel:`, `geo:`) are open,
published standards.
