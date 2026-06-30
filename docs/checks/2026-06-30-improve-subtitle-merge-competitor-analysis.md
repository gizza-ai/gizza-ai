# subtitle-merge — competitor analysis & surface checks (2026-06-29)

**Tool:** `subtitle-merge` — merge multiple SRT/VTT subtitle files with optional cumulative time shift.

## Verification snapshot

Verified on 2026-06-30 (CARGO_BUILD_JOBS=1).

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/subtitle-merge && cargo test --workspace` | ✅ 16 passed (15 core + 1 schema drift guard; web 0) |
| Wafer block | `cd blocks/subtitle-merge && wafer build` | ✅ OK gizza-ai/subtitle-merge v0.1.0 (315.2 KiB) |
| Wafer fixtures | `for f in tests/*.json; do wafer test "$f"; done` | ✅ 3/3 pass (merge-basic, offset, force-vtt) |
| Web build | `wasm-pack build blocks/subtitle-merge/web --target web --release --out-dir pkg` | ✅ pkg built |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered tools/subtitle-merge/ (289 tools) |
| CLI | `gizza tool subtitle-merge subtitles=… offset_ms=… format=…` | ✅ merge/sort/renumber + offset + force-vtt all correct |
| Page (Playwright) | `cd tests && xvfb-run npx playwright test tool-page-subtitle-merge.spec.ts` | ✅ 4/4 passed (SRT merge, cumulative offset, force VTT, query-param deep-link) |

## Competitor scan

Representative tools and feature patterns:

1. **SubtitleTools / Happy Scribe-style online mergers** — upload or paste multiple SRT files and download a combined subtitle track.
2. **VEED / Kapwing subtitle utilities** — browser workflows for combining captions, shifting timing, and exporting SRT/VTT alongside video editing.
3. **Subtitle Edit desktop app** — robust subtitle editing including append/merge, renumbering, sorting, shifting, and conversion between subtitle formats.
4. **ffmpeg / command-line workflows** — concatenate or remux captions, but usually require shell familiarity and separate timestamp adjustment.
5. **WebVTT/SRT converters** — focus on output format conversion and timestamp separator/header normalization.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Merge multiple subtitle files | Common subtitle tools | ✅ pasted files separated by `===` |
| Stable time sorting and renumbering | Desktop subtitle editors | ✅ all cues sorted by start time and renumbered |
| Overlay tracks on same timeline | Bilingual/alternate caption workflows | ✅ `offset_ms=0` preserves equal-start input order |
| Append split parts with shift | Subtitle editors | ✅ cumulative `offset_ms` shifts file 2, 3, ... |
| SRT and WebVTT support | Common converters | ✅ parses both and outputs `auto`, `srt`, or `vtt` |
| Browser-local privacy | Some web tools, but not always | ✅ pure Rust/WASM, no upload |
| Visual waveform/video sync | Video editors | Out of scope: text-only local caption merge |
| File upload/download UI | Dedicated web apps | Not built in this first pass; paste-based input fits current gizza text page pattern |

## Notes

The separator-line input keeps the tool simple and deterministic while supporting both overlay and sequential-part workflows. Cue settings and original numbering are normalized in the merged output.
