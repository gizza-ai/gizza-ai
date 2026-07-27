# video-audio-track-selector competitor analysis (2026-07-27)

Goal: keep exactly one chosen audio stream from a multi-audio video and remove the other audio streams, without re-encoding.

## Scan

| Reference | Table-stakes observed | In model? | Decision |
| --- | --- | --- | --- |
| FFmpeg stream-mapping recipes for removing audio tracks | Use explicit `-map` rules; keep video with `-c copy`; target a specific audio stream by index; fail clearly if a requested stream is missing. | Yes | Core plan maps all video streams plus `0:a:<track>` and uses `-c copy`; invalid indices surface ffmpeg's no-stream error. |
| Online mute/remove-audio tools | Browser/file-first UX, no upload messaging, simple presets/examples, clear output download, preserved input container where possible. | Yes | Page is an ffmpeg file tool with video upload, preset chips for first/second track, generic privacy copy, and output container matching the input extension. |
| Video editing/transcoding suites with audio-track controls | Often expose track language/name discovery, preview playback, keep/remove multiple streams, subtitle preservation options, and default-track disposition. | Partial | Track discovery/preview and arbitrary multi-track selection are out-of-model for the current page driver; `keep_subtitles` and default disposition are in-model and implemented. |

## Parameter and UX decisions

- `track` (integer, default `0`): in-model. Required table-stake because users must choose which audio stream survives. The UI labels it as 0-based and provides first/second-track preset chips.
- `keep_subtitles` (boolean, default `false`): in-model. Competitor suites often offer stream preservation; this tool keeps the primary default simple but lets users keep embedded subtitle streams with optional `0:s?` mapping.
- `set_default` (boolean, default `true`): in-model for CLI/chat descriptor. It flags the kept output audio stream as default; the page keeps it on to avoid an extra rarely-used control.
- Track language/name detection: out-of-model for the generic ffmpeg page because it would require probing the uploaded file and dynamically rewriting controls before running.
- Preview/listen before export: out-of-model for the static tool page; users can identify streams with VLC/ffprobe before upload.
- Keeping multiple audio streams: intentionally out-of-scope for this tool. The backlog item asks to keep a single chosen audio track and drop the rest.

## Worked examples to cover

- Keep the first audio track: `track=0`, no subtitles.
- Keep the second audio track from a two-audio fixture: `track=1`, no subtitles.
- Non-default checkbox state: `keep_subtitles=true` maps optional subtitle streams with `0:s?`.

## Verification plan

- Core unit tests assert exact ffmpeg argv for default, second-track, subtitle, and no-default-disposition cases.
- CLI test runs the tool against a two-audio MP4 fixture and uses `ffprobe` to confirm the output has exactly one audio stream while preserving 128x128 video.
- Page test imports the generated web module to assert exact argv and runs the generated page with a deep-link for `track=1`, then decodes the output video metadata.
