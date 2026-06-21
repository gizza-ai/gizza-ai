# wifi-qr-code-generator — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/wifi-qr-code-generator` — generate a Wi-Fi join QR code from
SSID, password and security type. Pure-Rust (`qrcode` → SVG). Text input → image
(SVG) output, so chat + CLI, no page (image-bytes output, like the chart tools).

## What competitors do

- **Online Wi-Fi QR makers** (qifi.org, many "wifi qr code generator" sites) —
  enter SSID + password, get a QR. Convenient, but you **type your Wi-Fi password
  into a third-party web page**, and some inject ads/branding into the image.
- **Router admin pages / phone "share Wi-Fi"** — built in on some devices, but not
  available everywhere and not scriptable.
- **`qrencode` + hand-built `WIFI:` string** — local and scriptable, but you must
  know the exact `WIFI:T:...;S:...;P:...;;` format and its escaping rules.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`qrcode`) compiled to wasm: the
   chat Service Worker and the CLI build the QR on-device. Your SSID and password
   never touch a network.
2. **Correct, standard payload.** Emits the exact `WIFI:T:<sec>;S:<ssid>;
   P:<pwd>;[H:true;];;` string phones recognise, with proper **escaping** of the
   special characters `\ ; , : "` in SSID/password (the part hand-rolled strings
   get wrong) — verified by unit tests.
3. **Handles the real cases.** WPA (covers WPA/WPA2/WPA3), WEP, and open
   (`nopass`, password omitted) networks, plus a `hidden` flag for non-broadcast
   SSIDs.
4. **Vector output.** A crisp, tiny SVG that scales to any size for printing on a
   card or poster; embeds directly in chat or a page.
5. **Agent- + CLI-friendly.** One call from chat or `gizza tool
   wifi-qr-code-generator ssid=… password=… --out wifi.svg`.

## Honest scope

- **Generates the QR; doesn't connect.** Scanning it with a phone camera offers to
  join the network — the connecting is done by the phone.
- **SVG output** (vector); not PNG (consistent with the other gizza
  QR/chart-producing tools — SVG is smaller and sharper, and trivially rasterised
  downstream).

## Tests

5 core unit tests: exact `WIFI:` payload for a WPA network; **special-character
escaping** in SSID and password (`;`, `:`, `\`); `nopass` omits the password and
still encodes the hidden flag; the SVG renders (well-formed `<svg>…</svg>`); and
error cases (missing SSID, missing password for WPA, open network needs none, bad
security). Plus the block drift-guard schema test. **CLI verified** end-to-end
(WPA and open networks → valid SVG QR codes). `wafer build` instantiates the chat
block (340 KiB).
