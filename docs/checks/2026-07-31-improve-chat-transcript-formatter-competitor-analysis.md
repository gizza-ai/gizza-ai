# chat-transcript-formatter — competitor analysis (2026-07-31)

Tool function: take a raw, messily-formatted chat log or conversation transcript
(WhatsApp export, IRC/Discord copy-paste, plain `Name:` lines, timestamped dumps)
and re-emit it as a single, consistently-formatted transcript preserving speaker,
time, and message.

Distinct from the existing `blocks/transcript-clean`, which is an ASR/caption
*prose cleaner* that STRIPS timestamps and speaker labels and removes filler
words. This tool does the opposite: it *parses and normalizes* structure and keeps
speaker + time. No overlap in intent.

## Scan (top competitors, paraphrased — no copy/branding reproduced)

1. **Sonix transcription-formatting guide** (sonix.ai/resources/transcription-formatting).
   Conventions: bolded speaker name followed by a colon; each speaker change is a
   new paragraph; timestamps in **square brackets** placed when the speaker
   changes; non-verbal cues in square brackets. Recommends a consistent format end
   to end (speaker labels, line breaks, timestamps stay uniform).
2. **Whisperbot conversation-transcription-example** (whisperbot.ai). Enumerates
   several output styles: (a) *timestamped dialogue* — `Name` + `HH:MM:SS`, one
   turn per line, consecutive same-speaker NOT merged; (b) *speaker-separated
   paragraph* — speaker label once, consecutive utterances consolidated into one
   block; (c) *annotated* — bracketed action/emotion notes; (d) *line-by-line* —
   each utterance its own line. Table-stakes: choice of output style, and a
   merge-consecutive-same-speaker option.
3. **Chat Viewer for WhatsApp / WhatsApp .txt readers** (Google Workspace
   marketplace listing; various). Reads an exported WhatsApp `.txt` (`[date, time]
   Name: message` or `date, time - Name: message`) and renders a clean, readable
   layout. Table-stakes: parse the two WhatsApp export line shapes, separate the
   date from the time, drop the noisy date by default, keep the time.
4. **ScreenApp / general transcript formatters** (screenapp.io/transcription/formatting).
   Upload plain text; auto-apply speaker labels + timestamps + paragraph breaks;
   let the user pick output layout; 12h vs 24h and bracketed-timestamp options are
   common.

## Table-stakes → decision (every item lands in the descriptor or the out-of-model list)

| Table-stake | In/out model | Decision |
|---|---|---|
| Parse WhatsApp bracket form `[date, time] Name: msg` | in | `input` parser form A |
| Parse WhatsApp dash form `date, time - Name: msg` | in | parser form B |
| Parse `[HH:MM] Name:` / `(HH:MM) Name:` timestamped | in | parser form C |
| Parse IRC/Discord `<Name> msg` and `[HH:MM] <Name> msg` | in | parser forms C/D |
| Parse bare-leading-time `HH:MM Name: msg` | in | parser form E |
| Parse plain `Name: msg` | in | parser form F |
| Fold wrapped/continuation lines into the previous message | in | continuation rule |
| Choice of output style (colon / markdown-bold / IRC-angle / screenplay) | in | `output_format` enum |
| Bolded speaker + colon (markdown) | in | `output_format = markdown` |
| Square-bracket timestamps | in | timestamp emitted as `[HH:MM]` prefix |
| 12h vs 24h timestamp normalization | in | `time_format` enum (`keep`/`24h`/`12h`/`none`) |
| Drop/keep timestamps entirely | in | `time_format = none` |
| Keep/drop the date (WhatsApp) | in | `include_dates` boolean (default off) |
| Merge consecutive same-speaker turns | in | `merge_consecutive` boolean |
| Paragraph break between turns | in | `blank_line_between` boolean |
| Non-verbal cue annotation `[laughs]` handling | in (passthrough) | messages kept verbatim; cues preserved |
| **Auto speaker diarization from audio** | out | needs an ML model — listed, not built |
| **Voice/audio-file input (Discord/Slack calls → text)** | out | needs speech-to-text model — listed, not built |
| **Translation / language detection across 90+ languages** | out | out of scope for a deterministic formatter |
| **Live WhatsApp text styling preview (bold/italic simulator)** | out | messaging-client styling preview, not a transcript formatter |
| **Cloud export to DOCX/PDF/SRT, account-based history** | out | server/account feature — browser-local tool outputs text |

## Notes
- Everything shipped is deterministic (fixed regex parsing + reassembly), no LLM,
  no network — consistent with gizza's browser-local wasm model.
- Ambiguity acknowledged on the page: a plain `Word: text` line is treated as a
  speaker label (this is inherent to plain chat logs); lines with no recognizable
  speaker/timestamp are folded into the previous message as continuations.
