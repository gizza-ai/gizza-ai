# device-mockup-framer — competitor analysis (2026-07-17)

Scan run **before** implementation to set table-stakes params + defaults. All observations are
**paraphrased** from public tool pages — no competitor copy, branding, or trademarks reproduced.
This tool is the pure-Rust `image`-crate family (image input → PNG bytes → chat + CLI, **no page**,
like `screenshot-beautify` / `image-border-frame`), so the "page/UX preset" gaps below are recorded
as informational, not built.

## Function searched
"device mockup generator online — frame a screenshot in a phone / tablet / laptop / browser window".

## Competitors skimmed (top 3 reachable real tools)

### 1. Screenhance — device / browser frame generator (screenhance.com)
- **Features:** large frame library (43 device frames, 100+ templates); a dedicated *browser*
  mockup mode that wraps a shot in a Chrome/Safari/Firefox-style window; a *device frame* mode
  (phone/tablet/laptop); multi-device compositions (phone beside laptop beside tablet).
- **Params/UX:** device picker, background (solid + gradient presets + image), padding, shadow.
- **Out-of-model:** multi-device composition (single input here), photoreal template library,
  hosted background-image library.

### 2. MockupShot — browser & phone frame tool (mockupshot.online)
- **Features:** upload a screenshot → wrap in a Chrome / Safari / Firefox / Edge browser frame;
  tweak background color; **light vs dark address bar**.
- **Params/UX:** browser-chrome style, address-bar theme (light/dark), background color.
- **Table-stakes captured:** browser window with a light/dark chrome + address bar → in-model.

### 3. ImgBite — screenshot mockup generator (imgbite.com)
- **Features:** three frame types spelled out — **Browser** (macOS-style window with traffic-light
  dots + a URL/address bar), **Phone** (modern smartphone with a notch), **Laptop** (screen housing
  with a base/deck). Background + shadow controls.
- **Params/UX:** frame type, background, shadow. This maps 1:1 onto our four device modes.
- **Table-stakes captured:** traffic-light dots, address bar, phone notch, laptop base → in-model.

## Table-stakes params (tagged for model fit)

| Param | Competitors | Fit | Decision |
| ----- | ----------- | --- | -------- |
| device type: phone / tablet / laptop / browser | all 3 | in-model | `device` enum (4 values) |
| frame/body color (black / white / silver) | Screenhance, ImgBite | in-model | `frame_color` enum |
| background: gradient / solid / transparent | all 3 | in-model | `background` enum + `bg_color`/`bg_color2`/`gradient_angle` |
| padding / margin around device | all 3 | in-model | `padding` px |
| drop shadow (+ intensity) | all 3 | in-model | `shadow` + `shadow_blur` + `shadow_opacity` |
| browser chrome: traffic-light dots + address bar | ImgBite, MockupShot | in-model | drawn in `browser` mode |
| address-bar URL text | MockupShot, ImgBite | in-model | `browser_url` (rendered with a bundled font) |
| light vs dark browser chrome | MockupShot | in-model | derived from `frame_color` (black → dark chrome) |
| phone notch / laptop base / tablet camera | ImgBite | in-model | drawn per device |
| **multi-device composition** | Screenhance | **out-of-model** | single image input; not built |
| **3D perspective / rotation** | Device Frames (deviceframes.com) | **out-of-model** | needs a 3D renderer; not built |
| **photoreal device template library** | Screenhance, iMockup | **out-of-model** | needs bundled hi-res device art; we draw clean vector-style bezels |
| **hosted background-image library** | Screenhance | **out-of-model** | no asset hosting; solid/gradient/transparent only |

## Feasibility spike (pure Rust / wasm, 5 min)
The `image` crate (already proven in `screenshot-beautify`) composes rounded-rect bezels, discs
(traffic lights / camera), gradients, and blurred drop shadows purely — no ffmpeg, runs on every
backend. Address-bar text is rasterized with the pure-Rust `fontdue` crate + a bundled DejaVu font
(already proven in `code-screenshot`). All in-model table-stakes are therefore buildable pure.

## Design descriptor (all in-model table-stakes included)
`device` (phone|tablet|laptop|browser), `frame_color` (black|white|silver), `background`
(gradient|solid|transparent) + `bg_color`/`bg_color2`/`gradient_angle`, `padding`, `shadow` +
`shadow_blur` + `shadow_opacity`, and `browser_url` (address-bar text for the browser mode).
