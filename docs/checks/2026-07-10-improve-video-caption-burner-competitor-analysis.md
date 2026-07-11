# video-caption-burner — competitor analysis (2026-07-10)

Tool function: hardcode (burn in) a timed subtitle track — SubRip `.srt` / WebVTT
`.vtt` — permanently onto a video, with styling. Distinct from the sibling
`video-title-card` (a single, one-window caption); this burns a *full multi-cue
track* with per-cue timings. Sibling `subtitle-merge` only merges subtitle files;
it never touches a video.

Competitor scan done BEFORE implementing. Paraphrased only — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed (top real, reachable tools)

Search: "burn subtitles into video online hardcode SRT free tool" (July 2026).
Two initially-picked tools were unreachable from this network (videotools.space —
DNS ENOTFOUND; happyscribe.com — DNS ESERVFAIL) and were replaced per the skill's
"replace unreachable competitors" rule.

1. **Loopaloo — Video Subtitle Burner.** Formats: SRT + VTT. Styling: typeface/weight,
   font size, text color, outline (called out for readability), background box with
   adjustable opacity, position top/center/bottom. Guidance (not a hard limit): two
   lines × ~40 chars is the readability standard.
2. **FixTools — Subtitle Burn-in.** Formats: SRT, VTT, ASS, SSA, or manual text entry.
   Styling: position bottom/top, font size (recommends ~20–32px), text color
   (white/yellow suggested), background color. No font-family, outline, or center
   position exposed. Limits: 50 MB free / 500 MB paid per video.
3. **EditClips — Burn Subtitles.** Formats: SRT, ASS, SSA, VTT. Styling: font size in
   tiers (small→extra-large), color choices (white/yellow/cyan/green), background mode
   (outline-only / dark box / none), position bottom/top/center. Output container
   choice (MP4/MKV/MOV/WebM) + quality tiers. Limits: 100 MB video, 10 MB subtitle file.

## Table-stakes → decision (every one lands in the descriptor or the out-of-model list)

| Table-stake | In gizza model? | Where |
| --- | --- | --- |
| SRT (`.srt`) input | yes | `subtitles` param (parser) |
| WebVTT (`.vtt`) input | yes | `subtitles` param (parser handles `.`/`,` + `WEBVTT` header) |
| Manual subtitle text entry (paste, no file) | yes | `subtitles` is a multiline text param — native |
| Font size | yes | `font_size` (px, default 24; competitors ~20–32) |
| Text color | yes | `font_color` (name or hex, default white) |
| Outline / border for readability | **out-of-model for v1** | browser + CLI use one consistent `drawtext` chain with a background box for contrast; separate outline controls would add more surface area to the already-long per-cue filter graph |
| Background box + opacity | yes | `background` + `background_color` + `background_opacity` |
| Position bottom / top / center | yes | `position` enum (all horizontally centered, subtitle convention) |
| Font family / typeface choice | **out-of-model** | one bundled Liberation Sans Bold face (browser ffmpeg has no system fonts) |
| ASS / SSA input with embedded styling | **out-of-model** | ASS carries its own style/position engine (libass); we render plain timed text only. `{...}` overrides and `<i>/<b>` tags are stripped to plain text |
| Output container choice (MKV/MOV/WebM select) | **out-of-model** | output keeps the input container when it can hold H.264+AAC, else MP4 (family-invariant with the other video tools); no arbitrary container picker |
| Quality / bitrate tiers | **out-of-model** | fixed libx264 `-preset medium`, family-invariant |
| Large uploads (50–500 MB) | **out-of-model** | 25 MiB cap — everything runs locally in-browser wasm; larger files are a server/paid feature |

## UX control patterns to match (competitors ship these)

- Position as a labelled `<select>` (bottom/top/center) → `Param::enumv` + `[input.labels]`.
- Font size as a slider → meta `kind = "slider"`.
- Colors as swatch+text → meta `kind = "color"`.
- Preset "try" chips (readability defaults, top placement, boxed captions) → `[[example]]`.

## Worked example (used on the page)

Input SRT:
```
1
00:00:00,000 --> 00:00:02,000
Hello there.

2
00:00:02,000 --> 00:00:04,000
This caption is burned in.
```
→ a bottom-centered white caption track with a semi-transparent black readability
bar hardcoded onto the video (H.264, container kept when it can hold H.264+AAC,
else MP4).

## Notes

- No competitor copy, branding, or trademarks reproduced — features/UX ideas only.
- Distinct from `video-title-card` (single caption/lower-third), `subtitle-merge`
  (merges subtitle files, no video), and `srt-shift`/`srt-to-vtt` (subtitle-only).
