# audio-ringtone — competitor analysis (2026-07-19)

New-tool build (backlog: "Trim, fade, and export an audio clip as a phone
ringtone (<=30s, m4r/mp3)"). Scan done BEFORE implementation; findings shaped
the descriptor. All paraphrased — no competitor copy/branding reproduced.

## Competitors skimmed (top 3 reachable)

1. **ringtonemaker.com** — waveform editor with draggable start/end handles;
   accepts mp3/m4a/aac/wav/flac/ogg up to 50 MB; exports m4r (iPhone) and mp3
   (Android) at 96 kbps mono; iPhone cap stated as 40 s; auto-applies ~0.5 s
   fade-in/out (anti-click), light compression and EBU R128 loudness
   normalization; upload → set range → preview → process → download flow;
   install instructions for Files/GarageBand/iTunes-Finder.
2. **mp3cut.net** — waveform with draggable interval handles + arrow-key
   nudge; "iPhone ringtone" one-click mode that exports m4r auto-capped at
   40 s; optional fade-in/fade-out; also volume/speed/pitch extras; guides
   for transferring to the phone.
3. **notevibes.com/ringtone-maker** — client-side (Web Audio) mp3 cutter;
   30 s framing presented as the default ringtone length with a compliance
   nudge (soft, not hard, cap); preset buttons for first/middle/last 30 s;
   drag handles on a waveform; mp3-only output (points iPhone users at
   GarageBand for m4r); install instructions for both platforms.

## Table stakes → decision

| Capability | Competitors | Fit | Where it landed |
|---|---|---|---|
| Trim via waveform drag-handles + numeric fields | all 3 | in-model | `[waveform] start/end` binding + `start`/`end` params |
| m4r (iPhone) export | 1, 2 | in-model (spiked: needs explicit `-f ipod`, ffmpeg can't infer from `.m4r`) | `format` enum, default `m4r`; platform `EXT_MIME` gained `m4r → audio/mp4` |
| mp3 (Android) export | all 3 | in-model | `format` enum value `mp3` |
| 40 s iPhone hard cap | 1, 2 | in-model | validation: `end − start ≤ 40`, error names the cap |
| 30 s default length | 3 (and backlog desc) | in-model | `end` 0/omitted → `start + 30` |
| Anti-click edge fades (~0.5 s auto) | 1 (auto), 2 (opt-in) | in-model | `fade_in`/`fade_out` sliders 0–5 s, default 0.5 |
| Loudness normalization (EBU R128) | 1 (auto) | in-model (spiked: `loudnorm=I=-14:TP=-1.5:LRA=11` + `-ar 44100`, fades after loudnorm) | `normalize` boolean, default true |
| Preview before download | all 3 | in-model (platform) | page `<audio>` output + shared waveform player |
| Preset chips | 3 (first/middle/last 30 s) | partly in-model (chips are static values — "middle/last" need the file's duration, which chips don't see) | 3 `[[example]]` chips: chorus-at-45s m4r, Android mp3 + 2 s fade-out, raw cut |
| Install how-to (Finder/iTunes/GarageBand; Android file manager) | all 3 | copy | 2 dedicated FAQs |
| Local/private processing | 3 (client-side) | in-model (platform) | stated in copy ("nothing is uploaded") |

## Out-of-model / not built (listed, not dropped)

- **Duration-relative presets** (first/middle/LAST 30 s buttons) — example
  chips prefill static param values and can't read the uploaded file's
  duration; the draggable waveform selection covers the same job.
- **Volume/speed/pitch extras** (mp3cut.net) — separate concerns already
  covered by dedicated blocks (`audio-volume-adjust`, `audio-time-stretch`,
  `audio-pitch-shift`); not table-stakes for a ringtone cut.
- **Compression + mono 96 kbps phone-speaker mastering** (ringtonemaker.com's
  fixed pipeline) — we keep the family-standard 192 kbps stereo encode;
  loudnorm's −1.5 dBTP ceiling covers the "loud but unclipped" need without
  an opinionated mono downmix.
- **50 MB / 10 GB uploads** — this repo's audio family caps inputs at 10 MiB
  (envelope/memory budget); stated on the page.
- **Server-side conversion of 300+ exotic formats** — input is whatever the
  browser ffmpeg build decodes (covers all common cases; stated on page).

## Verification notes

- ffmpeg chain spiked locally before implementation: trim → loudnorm →
  afade/areverse fades → `-ar 44100 -c:a aac -b:a 192k -f ipod out.m4r`
  produced a valid 44.1 kHz AAC ipod-container file (ffprobe-verified;
  mean volume −14.4 dB from a −30 LUFS source). Without `-f ipod`, ffmpeg
  fails on the `.m4r` extension — locked in by a unit test.
- Page mime table (`tools/generator/assets/runtime/tool-ffmpeg.js`) had no
  `m4r` entry → output would have rendered `application/octet-stream` and
  broken the audio preview; fixed platform-wide (`m4r: "audio/mp4"`).
