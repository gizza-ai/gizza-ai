# video-duration-validator — competitor analysis (2026-08-23)

Scan run before finishing the tool. Query: `online video duration checker target length tolerance`. The top results clustered into three real capability profiles: media metadata inspectors, ad/social-duration validators, and general video validators that include duration as one rule. Everything below is paraphrased; no competitor copy, branding, trademarks, or assets are reused.

## Competitors skimmed

| # | Tool profile | Reachable? |
| - | ------------ | ---------- |
| 1 | Browser video metadata inspector showing duration, codec, dimensions, bitrate, and frame rate after local file selection | yes |
| 2 | Online video validator for ad/social specs with maximum duration, file size, aspect ratio, and codec checks | yes |
| 3 | Simple duration/length checker that reports actual runtime from a selected video or audio file | yes |
| 4 | Desktop/pro workflow using ffprobe-style metadata and a scriptable threshold comparison | yes, via docs/examples |
| 5 | Upload-based video QA services with presets for platforms and delivery specs | skimmed; backend/account features are out of scope |

## Table stakes observed

| Capability | Typical shape | Default / pattern |
| ---------- | ------------- | ----------------- |
| Actual duration | seconds plus human timecode | derived from container metadata |
| Target length | number or preset | 6s, 15s, 30s, 60s, 180s, 10m examples |
| Tolerance | number | 0–1s; often implicit in ad tools |
| Rule mode | exact/within, maximum, minimum | within/exact for fixed ads; max for platform caps |
| Verdict | pass/fail or valid/invalid | clear status with reason |
| Delta | actual minus target | helps decide trim/pad amount |
| Container support | MP4/MOV/WebM plus audio containers | depends on parser |
| Local privacy | browser-local for simple inspectors | upload/account for deeper QA suites |
| Extra checks | codec, resolution, bitrate, aspect, file size | useful but outside this slug's duration-only scope |
| Presets | common ad/social lengths | chips or dropdowns |

## Shipped in-model

- Local WebAssembly duration read from container metadata via the existing pure-Rust media-info core.
- Video URL/ref chat and CLI surface, with page file selection through a small custom adapter.
- `target_seconds` required, `tolerance_seconds` default `0.5`, and `mode` enum: `within`, `max`, `min`.
- JSON result with `status`, `pass`, `reason`, `actual_seconds`, `actual_duration`, target/tolerance, delta, overshoot, allowed bounds, container label, and summary.
- Preset chips for common fixed and maximum/minimum rules.
- Validation for non-finite/negative/oversized targets and tolerances, and a clear error when the container records no usable duration.

## In-model but intentionally not included

- Codec, bitrate, dimensions, aspect-ratio, and file-size checks. Those are separate validation dimensions; packing them into this duration tool would bloat the schema and duplicate existing metadata tools.
- Batch validation. The current page pattern is one file at a time, and the CLI can already be scripted over multiple files.
- Platform-specific named presets as authoritative policy. Platform limits change; example chips are starting points, not a maintained compliance database.

## Out-of-model

- Server-side transcoding, frame-accurate decode counts, or repairing broken headers. The tool reads container duration; damaged files should be remuxed first.
- Upload/account workflows, saved reports, team review queues, or signed compliance certificates.
- Live API checks against a delivery portal.

## Positioning

This tool is a scriptable duration gate rather than just a metadata viewer: it returns a direct PASS/FAIL and the exact amount outside the allowed window, while keeping the same logic available on the page, CLI, and chat surfaces.
