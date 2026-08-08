# Competitor analysis — wav-to-raw-pcm-extractor

Date: 2026-08-08
Tool: `wav-to-raw-pcm-extractor`
Backlog prompt: Strip the WAV header and chunks to output the bare interleaved PCM sample bytes.

## Sources scanned

- Toolpkg `wav-to-pcm`: positions the task as stripping a WAV/RIFF wrapper and producing headerless PCM for DSP, game engines, and embedded playback.
- ffmpeg / Stack Overflow raw PCM discussions: raw output is selected by naming the raw muxer/sample format (for example `s16le`) and, when re-importing raw bytes, the user must supply sample rate, channel count, and format because the header is gone.
- SoX / audio repair discussions: common workflows either copy/trim a WAV to normalize chunks or convert a WAV to raw then wrap it again with corrected metadata.
- Raw PCM reference material (for example FileDex/format notes): table-stakes warnings are that `.pcm` has no embedded sample rate, channels, endianness, or bit depth, so those settings must be carried next to the bytes.
- General audio converters (XConvert-style): users expect codec/sample-rate/channel controls and worked examples, but broad audio transcoding is outside a pure WAV-data-chunk stripper.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor expectation | Gizza decision |
| --- | --- | --- |
| Strip the RIFF/WAVE wrapper and return only sample bytes | Core task: remove header and metadata chunks | In model. Implemented as a chunk walker that finds the first `data` chunk and slices it byte-for-byte by default. |
| Preserve payload exactly | DSP/firmware users often need no decode/re-encode | In model. `sample_format=source` + `channels=all` is the default and is bit-identical. |
| Name raw PCM settings needed for playback | Raw PCM has no header, so users need sample rate, channel count, bit depth, byte order | In model. `output=info` reports sample rate, channels, encoding, frame count, chunk map, data offset/size, and ffmpeg/SoX/Audacity re-import commands. |
| Common output renderings | Online tools usually offer a download; developer workflows need hex/C arrays | In model for text surfaces. Outputs are base64, grouped hex, C array, or info report. Direct binary download from chat/page is out of model for this pure text tool, but base64 can be decoded losslessly. |
| Input as file upload | Online converters commonly upload or browser-select a file | Out of model for this block/page shape: the generic text page accepts strings, not binary uploads. We accept base64 or hex and document how to encode a file. |
| Sample format selection | ffmpeg/SoX workflows use names like `u8`, `s16le`, `s24le`, `f32le` | In model. Enum choices: source, u8, s16le/be, s24le/be, s32le/be, f32le/be. |
| Endianness | Raw import/export needs little vs big endian | In model. Explicit BE/LE sample formats; WAV source is read as little-endian. |
| Channel selection/downmix | Converters often expose mono/stereo/channel controls | In model. `channels=all`, `mono`, `left`, `right`; mono averages channels and right errors on mono clips. |
| Trim/window | Users extracting a raw segment need offsets | In model. `start_frame` and `max_frames`, using sample frames rather than byte offsets to avoid splitting channels/samples. |
| Line wrapping / firmware-friendly output | Hex dumps and C arrays need wrapping controls | In model. `line_bytes` controls hex and C array wrapping, including `0` for an unbroken hex stream. |
| Compressed formats (MP3/FLAC/Ogg/AAC) | Broad converters accept many audio codecs | Out of model for a WAV data-chunk stripper. The tool sniffs common non-WAV containers and asks the user to convert to WAV first. |
| A-law / mu-law conversion | Some WAVs contain companded telephony samples | Partly in model. Verbatim extraction works because it is just bytes; linear PCM conversion is out of model and errors clearly. |
| Privacy/no upload | Browser tools often advertise local processing | In model. Page copy states WebAssembly/local execution and no upload. |

## Descriptor / page choices

- Textarea for the WAV byte string so long base64 or hex wraps cleanly.
- Enum selects for input encoding, output format, sample format, and channel selection.
- Slider for `line_bytes` because competitors/dev tools expose wrapping presets and the valid range is bounded.
- Example chips cover default base64 extraction, hex output, info report, u8 conversion, stereo-left extraction, and firmware C-array output.
- Worked examples use tiny generated WAV fixtures with exact outputs so users can verify the tool and tests can assert real values.

## Verification focus

The test matrix must prove:

- Exact default output from a base64 WAV.
- Hex input as a secondary input format.
- Every output mode advertised: base64, hex, C array, info.
- Sample format conversion (`u8`, BE/LE paths in core tests).
- Channel selection including left/right/mono and mono error handling.
- Frame windowing and cap/error paths.
- Page deep-link parameters trigger a real run and exact output.
