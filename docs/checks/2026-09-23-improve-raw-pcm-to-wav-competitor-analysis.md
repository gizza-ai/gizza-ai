# raw-pcm-to-wav — competitor analysis (2026-09-23)

Scope: wrapping **headerless raw PCM** in a RIFF/WAVE header, given the parameters the bytes
themselves do not carry (sample rate, bit depth, signedness, endianness, channel count).
Paraphrased notes only — no competitor copy, branding or trademarks are reused anywhere in the
block, the page, or the tests.

## Duplicate check (done before implementing)

| Existing block | Why it is NOT this tool |
| --- | --- |
| `wav-to-raw-pcm-extractor` | The exact **inverse**: it strips a RIFF header off a WAV and returns the bare `data` chunk. It never constructs a header. The two are complements, and its `info` output even prints the ffmpeg/SoX re-import parameters this tool consumes. |
| `sphere-to-wav` | Input is a NIST SPHERE file, i.e. audio that **already carries** an ASCII header describing itself. The whole point of raw PCM is that no such header exists and the user must supply the parameters. |
| `aiff-to-wav`, `audio-convert`, `base64-to-audio-file` | All require a self-describing container (AIFF / any ffmpeg-probeable format / an already-valid audio file in base64). Raw PCM has no magic bytes and fails probing in every one of them. |
| `wav-samples-to-json`, `wav-to-csv-samples`, `bitcrush`, `audio-bit-depth-converter` | Read or transform decoded audio; none accept headerless bytes plus a format description. |

Conclusion: **not a duplicate — build it.**

## Competitors reviewed

### 1. Audacity — *File ▸ Import ▸ Raw Data* (desktop, the reference GUI for this job)

The dialog is the de-facto specification of what a raw-PCM importer must ask for. Fields observed:

- **Encoding** — a combined list (signed 8/16/24/32-bit PCM, unsigned 8-bit PCM, 32-bit and
  64-bit float, U-Law, A-Law, plus GSM/ADPCM variants).
- **Byte order** — little-endian / big-endian (little-endian is the usual answer for anything
  produced on a PC).
- **Channels** — 1 (mono), 2 (stereo), and higher counts.
- **Start offset** — in bytes, for skipping a proprietary header or leading junk.
- **Amount to import** — as a percentage of the file.
- **Sample rate** — typed in Hz.
- A **Detect** button that guesses the settings.

### 2. ffmpeg — `ffmpeg -f s16le -ar 44100 -ac 2 -i in.pcm out.wav`

The raw PCM demuxers are named by a single token that fuses bit depth, signedness and byte order:
`u8`, `s8`, `s16le`/`s16be`, `s24le`/`s24be`, `s32le`/`s32be`, `f32le`/`f32be`, `f64le`/`f64be`,
`mulaw`, `alaw`. Rate and channel count come from `-ar` and `-ac`; a window comes from `-ss`/`-t`.
Terse and exact, but every parameter is mandatory knowledge and a wrong `-f` silently produces
noise rather than an error.

### 3. SoX — `sox -t raw -r 44100 -b 16 -e signed-integer -L -c 2 in.raw out.wav`

Splits the same information across orthogonal flags: `-r` rate, `-b` bits, `-e` encoding
(`signed-integer`, `unsigned-integer`, `floating-point`, `u-law`, `a-law`), `-L`/`-B` endianness,
`-c` channels. This is the split we copy in spirit, because each knob is independently explainable
on a web form in a way a fused `s24be` token is not.

### 4. (Context) Web converters — FreeConvert *raw-audio to WAV*, Descript, Uberduck

Checked because they rank for the query. They expose editing options (volume, trim, fade, codec
auto/copy) but **never ask for sample rate, bit depth, signedness or endianness** — they assume
16-bit 44.1 kHz. For genuinely headerless data that assumption is exactly the failure mode users
hit: the file "converts" and comes out as static or at the wrong speed. FreeConvert's own help
text correctly tells people to use Audacity, ffmpeg or SoX instead. That gap is this tool's reason
to exist: the parameters are first-class, and the result is previewable immediately.

## Table-stakes matrix → decisions

| Capability | Audacity | ffmpeg | SoX | Fit | Shipped as |
| --- | --- | --- | --- | --- | --- |
| Sample rate | yes | `-ar` | `-r` | in-model | `sample_rate` (integer, default 44100, 1–768000) |
| Channel count | yes | `-ac` | `-c` | in-model | `channels` (integer, default 2, 1–16) |
| Bit depth 8/16/24/32/64 | yes | in `-f` | `-b` | in-model | `bit_depth` enum `8\|16\|24\|32\|64` |
| Signed / unsigned / float | yes | in `-f` | `-e` | in-model | `encoding` enum `signed\|unsigned\|float` |
| G.711 mu-law / A-law | yes | `mulaw`/`alaw` | `-e u-law/a-law` | in-model | `encoding` enum `mulaw\|alaw` |
| Byte order | yes | in `-f` | `-L`/`-B` | in-model | `byte_order` enum `little\|big` |
| Start offset in bytes | yes | — | — | in-model | `skip_bytes` (integer, default 0) |
| Import only part of the data | `%` | `-t` | `trim` | in-model | `max_frames` (frames, default 0 = all) |
| Paste bytes as base64 / hex / `data:` URI | — | — | — | in-model | `input` + `input_format` (`auto\|base64\|hex`) |
| Get the result as a playable file | yes | file out | file out | in-model | `output=data_url` (`data:audio/wav;base64,…`) |
| Get the result as text for scripting | — | file out | file out | in-model | `output=base64` / `output=hex` |
| Explain what the numbers imply before writing | Detect | — | — | in-model | `output=info` — duration, frame count, byte budget, leftover partial frame, the exact header that will be written, and ready-to-run ffmpeg / SoX / Audacity equivalents |
| One-click presets for common dumps | — | — | — | in-model | five `[[example]]` chips (CD stereo, 8 kHz mono telephony, big-endian, 32-bit float, info report) |
| Auto-**detect** the format from the bytes | yes (Detect) | — | — | **out-of-model** | Not built. A real detector needs statistical scoring across ~20 candidate layouts; instead `info` reports the derived duration and any leftover partial frame, which catches the common "wrong frame size" mistake. Listed, not silently dropped. |
| Resampling / rate conversion | yes | `-ar` out | `rate` | **out-of-model here** | Deliberately out of scope: this tool re-containers bytes without touching samples. `blocks/audio-resampler` already does rate conversion. |
| GSM / IMA-ADPCM / MS-ADPCM sources | yes | yes | yes | **out-of-model** | Those are compressed, not linear PCM; decoding them is a codec, not a header. Rejected with a message naming the limitation. |
| Volume / trim / fade editing | — | filters | effects | **out-of-model** | Covered by the existing `audio-volume-adjust`, `trim-audio`, `audio-fade` blocks. |
| File-picker upload | yes | n/a | n/a | out-of-model on this page | The page takes pasted base64/hex like every other pure byte tool in the toolkit; the wasm runs locally so nothing is uploaded either way. |

## UX patterns adopted

- Orthogonal controls (SoX's split) rather than a fused `s16le` token, with friendly
  `[input.labels]` on every enum so the select reads "Signed integer (two's complement)" instead
  of `signed`.
- Preset chips for the four dumps people actually have: CD-style 44.1 kHz/16-bit/stereo, 8 kHz
  mono telephony, a big-endian capture, and 32-bit float — the web converters ship presets, so we
  do too.
- An `info` mode that prints the equivalent ffmpeg / SoX / Audacity settings, because the most
  common next step is to reproduce the conversion in a pipeline.
- Errors that name the fix: an invalid depth/encoding pairing says which depths that encoding
  allows, and a leftover partial frame is reported with the exact byte count.
</content>
</invoke>
