# wav-to-numpy-npy — competitor analysis (2026-08-21)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased from public API documentation; no competitor copy, branding, or
trademarks are reproduced here or on the page.

## What the tool has to be

"WAV → `.npy`" has no meaningful *web-tool* competitor set — nobody ships a browser page for it.
The real competitors are the **Python library calls people run instead**, and the **file-format
spec** the output must satisfy. Those are what the tool is benchmarked against, so the scan
targeted them:

1. **`scipy.io.wavfile.read`** — the de-facto "WAV to array" reference.
2. **NumPy `.npy` format spec** (`numpy.lib.format`) — the output container we must write byte-correct.
3. **`soundfile.read`** (python-soundfile / libsndfile) — the dtype/shape/windowing option surface.
4. **`librosa.load`** — the ML-pipeline default (checked as a 4th reference; its doc page 404'd on
   fetch, so its behaviour is taken from the widely-documented defaults: `sr=22050` resample,
   `mono=True` downmix, `dtype=float32`, `offset`/`duration` windowing).

The competing "workflow" is a two-liner: `sr, data = wavfile.read(f)` then `np.save(out, data)`.
Our tool has to produce a file that is byte-identical in spirit to that — `np.load()` must return
the same values, dtype, and shape — while additionally offering the dtype/shape/window controls
that make people reach for `soundfile` or `librosa` instead.

## Table-stakes, and where each one landed

| # | Table-stake behaviour | Reference | Verdict | Where it landed |
|---|---|---|---|---|
| 1 | Output is a real `.npy`: `\x93NUMPY` magic, version bytes, LE header-length, `descr`/`fortran_order`/`shape` dict sorted alphabetically, `\n`-terminated, space-padded to a 64-byte boundary | npy spec | **in-model** | core `write_npy()`; asserted by unit tests and cross-checked against the existing `npy-array-decoder` block |
| 2 | dtype follows the source: u8 → `uint8`, 16-bit → `int16`, 24/32-bit → `int32`, f32 → `float32`, f64 → `float64`; 8-bit unsigned, ≥9-bit signed | `scipy.io.wavfile.read` | **in-model** | `dtype = "auto"` choice (scipy-compatible; raw stored values, no normalisation) |
| 3 | Explicit dtype override from a fixed set | `soundfile.read` (`float64`/`float32`/`int32`/`int16`) | **in-model** | `dtype` enum: `auto`, `float32`, `float64`, `int16`, `int32`, `uint8` |
| 4 | Float dtypes are normalised to ≈[-1, 1]; int dtypes carry the full integer range | soundfile / librosa | **in-model** | float choices normalise, int choices scale to the target full scale — documented on the page and in each `describe()` |
| 5 | Shape is 1-D for mono, `(frames, channels)` for multichannel | scipy + soundfile default | **in-model** | `shape = "auto"` (the default) |
| 6 | `always_2d` — force `(frames, 1)` even for mono | `soundfile.read(always_2d=True)` | **in-model** | `shape = "frames_channels"` |
| 7 | Channels-first `(channels, frames)` layout | torch/torchaudio convention, reachable in NumPy via `.T` | **in-model** | `shape = "channels_frames"` |
| 8 | Flat interleaved 1-D regardless of channel count | raw-buffer workflows | **in-model** | `shape = "flat"` |
| 9 | Mono downmix | `librosa.load(mono=True)` | **in-model** | `mono` boolean (default **false** — we preserve channels, unlike librosa, because a lossy downmix should be opt-in) |
| 10 | Read a window instead of the whole file | `soundfile.read(start=, frames=)`, `librosa.load(offset=, duration=)` | **in-model** | `start_frame` + `max_frames`, matching the sibling WAV blocks' vocabulary (frames, not seconds) |
| 11 | Fortran (column-major) ordering flag in the header | npy spec `fortran_order` | **in-model** | `fortran_order` boolean (default false = C order, what `np.save` writes for a freshly-read array) |
| 12 | Tell the user the sample rate — it is **not** in the `.npy` | scipy returns `(rate, data)` as a tuple | **in-model** | `output = "info"` report prints the rate + a ready-to-run `np.load` snippet; also called out in the FAQ, because this is the single biggest footgun of the format |
| 13 | Refuse compressed/companded input clearly rather than guessing | scipy explicitly rejects mu-law/A-law | **in-model** | container sniffing names MP3/FLAC/Ogg/M4A/AIFF; A-law and mu-law WAVs error by name |
| 14 | 24-bit is widened, not truncated | scipy: "left-justified into the smallest compatible int type" | **in-model** | 24-bit `auto` → `int32`, and the value is left-justified (`<< 8`) exactly as scipy does |

### Out-of-model (listed, deliberately not built)

- **Resampling** (`librosa.load(sr=22050)`) — needs a resampler; the repo already ships
  `blocks/audio-resampler` for that, so this block keeps the native rate and says so.
- **Compressed input** (MP3/FLAC/Ogg/M4A) — this is a `pure` block with no ffmpeg; those go
  through `blocks/audio-convert` first. Consistent with every sibling WAV block.
- **`.npz` / multiple arrays in one file**, and pickled object arrays — a different container; the
  row asks for `.npy`.
- **Returning the sample rate *inside* the array file** — impossible: `.npy` stores exactly one
  array and no metadata. Surfaced in the `info` report instead (see #12).
- **File upload / drag-and-drop** — the page surface for pure byte tools is a pasted base64/hex
  string, same as `wav-to-csv-samples` and `wav-to-raw-pcm-extractor`.

## UX control patterns worth copying

- Competitors here are libraries, so there is no visual UX to match — but the **sibling gizza
  pages** set the bar, and the scan of `wav-to-raw-pcm-extractor` / `wav-to-csv-samples` gives the
  pattern to match: `[input.labels]` friendly `<select>` labels, `[[example]]` preset chips for
  every headline mode, a multiline base64 field with a real runnable placeholder, and an `info`
  output that prints the re-import command a headerless artifact needs.
- Every enum choice therefore ships a preset chip (defaults, scipy-compatible dtype, int16,
  channels-first, mono downmix, info report), so the whole option surface is one click away.

## Decisions taken into the descriptor

- Default `dtype = float32` (the ML-pipeline default, matching librosa) rather than soundfile's
  `float64` — half the bytes, and base64 output size matters in a browser.
- Default `shape = auto` and `mono = false` — preserve what the file actually contains; every
  lossy or reshaping step is opt-in.
- `output` = `base64` (default) / `hex` / `info`, mirroring `wav-to-raw-pcm-extractor` so the two
  WAV byte-export tools behave the same way.
- Frame windowing is capped (`max_frames`) with an explicit byte cap on the emitted `.npy`, since
  the whole array is materialised in the wasm sandbox.

Sources consulted: SciPy `io.wavfile.read` reference, NumPy `lib.format` format specification,
python-soundfile `read` reference, plus published `librosa.load` defaults.
