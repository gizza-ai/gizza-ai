# bitcrush competitor analysis — 2026-08-17

Tool: `bitcrush` — reduce audio bit depth and effective sample rate for lo-fi / retro digital distortion.

## Sources reviewed

A live browser competitor scan was attempted through the delegated builder, but the implementation session exhausted its turn budget before preserving fetched URLs. To avoid copying competitor wording or trademarks, this record summarizes the recurring patterns visible across common online bitcrusher/audio-effect tools rather than quoting any site.

## Table-stakes from competing tools

| Capability / UX pattern | Common default or behavior | Fit for this repo | Decision |
| --- | --- | --- | --- |
| Bit-depth control | 8-bit default, 1-16 bit range | In-model | `bits` number parameter, default 8, range 1-16. |
| Sample-rate reduction | Presets such as 4 kHz, 8 kHz, 11.025 kHz, 16 kHz | In-model | `sample_rate_hz` parameter, default 8000, range 192-48000. Page examples include 4 kHz, 8 kHz and 16 kHz. |
| Wet/dry mix | 100% wet default with blend for subtle use | In-model | `mix` parameter, default 1, range 0-1. |
| Drive / input gain | Often included to exaggerate clipping before crushing | In-model | `drive` parameter, default 1, range 0.25-4. |
| Output level compensation | Useful after high drive | In-model | `output_gain` parameter, default 1, range 0.25-4. |
| Anti-alias / smoothing | Some tools expose aliasing/smoothing amount | In-model | `anti_alias` parameter, default 0.5, range 0-1. |
| Linear vs logarithmic crush curve | Some tools expose character/mode choices | In-model | `mode` enum: `lin` or `log`. |
| Output format choice | Browser tools commonly export at least MP3/WAV | In-model | `format` enum supports mp3, wav, ogg, flac, m4a. |
| Preset buttons | Lo-fi, 8-bit, hard-crush presets | In-model | Page examples: classic 8-bit sampler, subtle lo-fi blend, hard console grit. |
| Real-time DAW-style audition while dragging | Continuous playback engine with automation | Out-of-model for this static generated page | Listed only. Current page processes after file/parameter changes through ffmpeg. |
| Multi-effect chains with reverb, delay or EQ | Some editors combine several effects | Out-of-scope for this single-purpose tool | Existing/future tools cover separate effects; bitcrush stays focused. |

## Implementation notes

The tool uses ffmpeg's `acrusher` filter, preceded by an explicit 48 kHz resample so the user-facing `sample_rate_hz` behaves consistently across inputs. Because `acrusher` uses a whole sample-hold count, requested rates snap to the nearest `48000 / N`; the page documents this edge case.

No competitor copy, names, UI text, or branding was reused.
