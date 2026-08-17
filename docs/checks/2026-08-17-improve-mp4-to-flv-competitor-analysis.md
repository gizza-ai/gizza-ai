# mp4-to-flv competitor analysis (2026-08-17)

Tool: `mp4-to-flv` — encode an MP4 or other ffmpeg-decodable video into an FLV stream for RTMP/Flash-era ingest.

## Sources scanned

- FreeConvert video converter: broad video conversion surface with optional advanced controls. Search result advertises video bitrate, resolution, and other quality settings.
- CloudConvert MP4 to FLV / FLV converter: MP4/WEBM/AVI input support and controls for video resolution, quality, and file size.
- 123Apps Online Video Converter (`video-converter.com`): browser upload flow with output format selection and controls for resolution/size; advertises uploads up to several GB.
- Convertio / similar online converters observed in search results: output format conversion plus optional resolution/codec/aspect-ratio controls.

The pages are server-side converters with upload/account/quota surfaces. This repo's model is different: local browser ffmpeg for page use, and a capped CLI/chat media envelope. No competitor copy, UI text, names, or branding is reused.

## Table-stakes capabilities

| Capability | Competitor pattern | In gizza model? | Decision for `mp4-to-flv` |
| --- | --- | --- | --- |
| Input video upload / URL | Pick local file or upload/import URL | Yes | Page accepts `video/*`; CLI/chat accept `url` or `ref` through `SourceFields`. |
| Output FLV container | Dedicated MP4→FLV conversion | Yes | Always writes `out.flv` and passes `-f flv` explicitly. |
| Video codec control | Most advanced converters expose codec/quality; FLV-specific tools often hide it | Yes | Expose `video_codec=h264|flv1`; default H.264, optional Sorenson Spark for legacy Flash. |
| Audio codec control | Advanced converters expose audio settings | Yes | Expose `audio_codec=aac|mp3|none`; MP3 pins 44.1 kHz because FLV rejects common 48 kHz MP3. |
| Resolution control | Common presets or size controls | Yes | Expose enum caps `source,1080p,720p,576p,480p,360p,240p`; no upscaling; even dimensions. |
| Frame-rate control | Common advanced setting | Yes | Expose `source,60fps,30fps,25fps,24fps,15fps`. |
| Video bitrate / quality | Common quality/file-size knob | Yes | Expose integer slider 100–20000 kbps, default 2500, used for `-b:v`, `-maxrate`, and buffer. |
| Audio bitrate | Common advanced setting | Yes | Expose integer slider 32–320 kbps, default 128. |
| Keyframe/GOP interval | Important for RTMP but rarely visible in consumer tools | Yes | Expose `keyframe_seconds` 1–10, default 2, via `-force_key_frames`. This is a deliberate RTMP-focused improvement. |
| Preset buttons | Many converters ship format/profile presets | Yes | Page ships preset chips for RTMP 720p, 1080p high bitrate, low bandwidth, legacy Flash, and video-only. |
| Target file size | Some tools optimize to a chosen file size | Partially/out-of-model | Not exposed: deterministic bitrate is simpler and fits the descriptor. Target-size solving would require duration probing and iterative encode in the page. |
| Batch conversion | Server tools often support queues | Out-of-model | One-file tool only; gizza pages run one wasm ffmpeg job at a time. |
| Cloud import/storage | Some tools import from cloud providers | Out-of-model | Not in the public toolkit model; page remains local-only. |
| Subtitle/multi-track handling | Some advanced converters preserve tracks | Out-of-model for FLV | FLV holds one video and one audio stream; first streams only, documented. |

## Defaults chosen

- Video codec: `h264` for modern RTMP and Flash 9.0.115+.
- Audio codec: `aac` for modern RTMP.
- Resolution/fps: `source` to avoid accidental quality loss.
- Video bitrate: `2500` kbps, a practical 720p-ish RTMP baseline.
- Audio bitrate: `128` kbps.
- Keyframe interval: `2` seconds, the common RTMP ingest recommendation.
- H.264 preset: fixed `veryfast`; browser ffmpeg needs predictable runtime.

## Worked examples to support

1. RTMP 720p ingest: H.264/AAC, 720p, 30 fps, 2500 kbps video, 2-second keyframes, 128 kbps audio.
2. Higher bitrate 1080p: H.264/AAC, 1080p, 30 fps, 5000 kbps video, 192 kbps audio.
3. Low bandwidth: H.264/AAC, 480p, 25 fps, 1000 kbps video, 96 kbps audio.
4. Legacy Flash: Sorenson Spark + MP3, 360p, 15 fps, 800 kbps video, 96 kbps audio.
5. Video-only stream: H.264, source size/rate, no audio.

## Gaps left out intentionally

- VP6 / Screen Video encoders: FLV can carry them historically, but wasm ffmpeg availability and browser runtime cost are not worth exposing until a concrete need appears.
- Target-size mode: useful but duration-dependent and less transparent than bitrate controls.
- Multi-file queues and cloud imports: not part of the current gizza local-tool model.
