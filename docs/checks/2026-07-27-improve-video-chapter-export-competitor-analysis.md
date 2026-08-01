# Competitor analysis: video-chapter-export

Date: 2026-07-27
Tool: `video-chapter-export`
Backlog description: Parses embedded chapter markers and exports them to a CSV/JSON/cue/text file without splitting the video.

## Sources reviewed

| Tool/source | What it does | Table-stakes observed | Fit decision |
| --- | --- | --- | --- |
| FFprobe / FFmpeg documentation and tutorials | Inspects media containers and can print chapter metadata in machine-readable writers such as JSON/XML/flat text. | Non-destructive metadata read; JSON output; start/end timestamps; works on many containers; CLI-first workflow. | In model: non-destructive extraction, JSON/text output, start/end timestamps. Out of model for this block: full FFmpeg demuxer breadth; this implementation supports the common MP4 `chpl` and Matroska/WebM chapter structures directly in Rust. |
| mp4chaps / MP4v2 chapter manager | Lists/imports/exports/removes MP4 chapters, including Nero/QuickTime chapter styles. | MP4-focused chapter list/export; text export; preserve video without re-encode; title + time markers. | In model: MP4 chapter export without splitting, plain text/CUE-friendly chapter lists. Out of model: editing/import/removal because the backlog item is export-only. |
| mkvextract / MKVToolNix | Extracts Matroska chapters to XML or simple OGM-style text. | MKV/WebM chapter extraction; preserves chapter titles; supports hierarchical chapter data; XML/simple text output. | In model: Matroska/WebM chapter extraction and title/start/end export. Out of model: full XML/hierarchical chapter authoring and all MKVToolNix editing modes. |
| JAD Apps video chapter extractor page | Online chapter extraction for LMS/content workflows. | Upload/paste source; export structured chapter list; no video splitting; human-readable output for course or chapter navigation. | In model: URL/ref source, no splitting, structured and human-readable formats. Out of model: hosted upload UX and LMS-specific integrations. |

## In-model requirements implemented

- Accept a video source by `url` or `ref` through the standard `Input::Video` descriptor.
- Sniff and parse MP4/M4V/M4A/MOV files with Nero-style `chpl` chapter atoms.
- Sniff and parse Matroska/WebM EBML chapter atoms.
- Preserve the source media unchanged; export only chapter metadata.
- Emit four fixed output formats via enum parameter: `json`, `csv`, `cue`, `text`.
- Include chapter index, title, start time, and end time when available.
- Normalize timestamps to milliseconds and include friendly `HH:MM:SS.mmm` strings for structured exports.
- Return a valid empty result for recognized containers with no chapter markers.
- Produce clear errors for unsupported containers and unknown formats.

## Out-of-model / intentionally not built

- Splitting videos at chapter boundaries: explicitly outside the backlog description and already covered by other media editing patterns.
- Importing, editing, removing, or writing chapter metadata back into a media file.
- Full FFmpeg-level support for every container and chapter metadata dialect.
- Hierarchical XML chapter export and all MKVToolNix-specific fields.
- LMS integrations, hosted upload storage, or branded web-app workflows.

## UX/control decisions

- `format` is an enum/select because competitors expose a small fixed set of export formats.
- Default format is `json` for predictable CLI/automation use.
- No standalone generated page/spec was added: this is a video file-input + text-output skill block that follows the existing no-page file-input pattern; CLI and chat schema are the locally verifiable surfaces.
- Output format coverage is verified with one real run per enum value in core integration tests.

## Worked example shape

Input: MP4/MKV with chapters `Intro` at 0s and `Demo` at 10s.

- `format=json` returns a JSON array in `content` with `index`, `title`, `start`, `end`, `start_ms`, `end_ms`.
- `format=csv` returns spreadsheet-ready columns: `index,start,end,start_seconds,end_seconds,title`.
- `format=cue` returns a CUE sheet with one `TRACK` per chapter.
- `format=text` returns YouTube-style lines like `0:00 Intro`.
