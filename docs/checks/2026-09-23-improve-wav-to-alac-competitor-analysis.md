# wav-to-alac — competitor analysis (2026-09-23)

Scan run BEFORE implementing, per `create-next-tool` step 4. Paraphrased observations only — no
competitor copy, branding, or trademarks are reproduced here or in the shipped page.

## Search

One web search: *"WAV to ALAC converter online Apple Lossless m4a settings sample rate bit depth"*.
Top reachable, genuinely comparable tools were skimmed (three, all reachable — no substitutions
needed):

1. **onlineconverter.com — WAV → ALAC** (server-side, upload + Convert button).
2. **conversion-tool.com — "Convert to ALAC"** (server-side, the most option-rich of the three).
3. **mymediatools.com — WAV → ALAC** (in-browser ffmpeg.wasm, zero-option).

## What each exposes

| Tool | Settings exposed | Defaults | Limits / notes |
| --- | --- | --- | --- |
| onlineconverter | none (file picker + Convert) | n/a | 200 MB upload cap; `.m4a` out; rejects DRM-protected input; server-side |
| conversion-tool | channels (unchanged/mono/stereo); bit depth (unchanged/16/24); sample rate (unchanged/8000…96000 Hz); trim start+end (HH:MM:SS); URL-or-file input; email notify | every audio selector defaults to "do not change" | 500 MB upload cap; no presets, no batch, no metadata controls; server-side |
| mymediatools | none | n/a | in-browser ffmpeg.wasm ("files never leave the device"); states ALAC-in-M4A, preserves source rate/depth/channels, maps source metadata where ffmpeg can; warns >2 channels is unreliable in Apple playback; batch lives on a separate page |

## Table stakes → decision

Every table-stake below ends in the descriptor or in the out-of-model list. Nothing dropped silently.

| Table stake | Seen on | Fit | Decision |
| --- | --- | --- | --- |
| ALAC in an `.m4a` container, `.m4a` extension | all 3 | in-model | Fixed output: `-c:a alac`, `out.m4a`, mime `audio/mp4` |
| Truly lossless (decoded PCM identical to source) | all 3 | in-model | ALAC encoder only; no bitrate/quality knob exists or is offered |
| Keep source sample rate / depth / channels by default | conversion-tool ("do not change"), mymediatools (implicit) | in-model | Every selector defaults to `source`, which omits the flag entirely |
| Bit depth 16 / 24 | conversion-tool | in-model | `bit_depth` = `source\|16\|24` → `-sample_fmt s16p\|s32p`. Verified locally: `s32p` makes ffmpeg report `bits_per_raw_sample=24`, `s16p` reports 16, and omitting it follows the source (16-bit WAV → s16p, 24-bit WAV → s32p) |
| Sample-rate selection | conversion-tool | in-model | `sample_rate` = `source\|44100\|48000\|88200\|96000\|176400\|192000` → `-ar`. Deliberately the music/Apple-Music-lossless family; telephony rates (8000–32000) are pointless for an archival lossless target and are documented as out of scope on the page |
| Channel selection (mono / stereo) | conversion-tool | in-model | `channels` = `source\|mono\|stereo` → `-ac 1\|2`. Also covers the >2-channel warning mymediatools raises: a surround WAV can be folded to stereo for Apple playback |
| Metadata carried into the M4A tags | mymediatools | in-model | `keep_metadata` boolean, default on → `-map_metadata 0`; off → `-map_metadata -1` for a clean, tag-free file |
| Runs locally / nothing uploaded | mymediatools | in-model | Already how the generated page works (ffmpeg compiled to WebAssembly, in-browser) |
| Faststart / streamable `.m4a` | implicit in all (Apple import) | in-model | Always `-movflags +faststart` so the moov atom is up front |
| URL input as an alternative to upload | conversion-tool | in-model | The chat/CLI surface already takes `url` or `ref`; the page keeps the file picker |
| Preset buttons for common targets | none of the three ship presets | in-model, cheap win | Added anyway as `[[example]]` chips: CD quality (16/44100), hi-res 24-bit, stereo fold-down |
| Trim start/end | conversion-tool | **out-of-model** | Trimming belongs to the existing `trim-audio` block; keeping this tool a pure converter avoids a duplicate |
| Batch / multi-file conversion | mymediatools (separate page) | **out-of-model** | The page file input is a single upload and the chat/CLI block takes one source; run the CLI in a shell loop |
| Email-when-done notification | conversion-tool | **out-of-model** | Server-side-queue feature; this tool converts synchronously and has no account system |
| 200–500 MB uploads | onlineconverter, conversion-tool | **out-of-model** | Server-side tools can stream to disk; an in-browser/wasm sandbox cannot. Cap is 25 MiB in / 60 MiB out, stated plainly on the page |
| DRM-protected input handling | onlineconverter | **out-of-model** | ffmpeg cannot decode protected streams; the error surfaces from ffmpeg as-is |

## UX control patterns adopted

- Three `<select>` dropdowns whose first option is the pass-through `source` value, matching the
  "do not change" idiom the option-rich competitor uses — a user who just wants a conversion
  changes nothing.
- A checkbox for metadata (default on), so the common case needs no interaction.
- `[[example]]` preset chips for CD quality / hi-res / stereo fold-down. None of the three
  competitors ship presets; the generator supports them, so this is a gap in *our* favour.
- Single-file upload with the generic drag/paste support the ffmpeg page runtime already provides.

## Gaps we intentionally do not close

Trim, batch, email notification, >25 MiB inputs, DRM. All listed above with reasons; none are
silently dropped, and none are hinted at in the page copy.
