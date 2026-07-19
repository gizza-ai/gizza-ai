## About this tool

**Transcript Cleaner** turns raw speech-to-text output, SRT/VTT captions, meeting notes, or
interview logs into cleaner prose. It strips timestamp scaffolding, removes filler words and
hyphenated stutters, drops non-verbal cue markers, merges consecutive turns from the same speaker,
and normalizes capitalization and punctuation.

Everything is deterministic and local: no LLM, no network call, and no upload. That makes the result
fast and repeatable, but it also means the tool uses fixed rules instead of understanding context.

### Worked example

**Raw transcript**

```
[00:01:23] Alice: um i think we should ship this
[00:01:26] Alice: uh it looks good [laughter]
Bob: like, agreed
```

**Cleaned transcript**

```
Alice: I think we should ship this. It looks good.
Bob: Like, agreed.
```

The default `standard` filler level removes unambiguous vocal pauses (`um`, `uh`, `erm`, `hmm`),
keeps the potentially meaningful word `like`, removes timestamps and `[laughter]`, then merges the
two adjacent Alice turns.

### Cleanup controls

- **Filler removal** — `off`, `standard`, or `aggressive`. Aggressive also removes discourse
  markers like `you know`, `I mean`, `basically`, `actually`, and `like`, which can be useful for a
  rough draft but may remove meaningful words.
- **Extra fillers** — a comma-separated custom list, applied at any filler level.
- **Remove timestamps** — drops SRT/VTT cue lines, sequence numbers, `WEBVTT`, bracketed clocks,
  and leading timestamps.
- **Remove bracketed cues** — strips markers such as `[laughter]`, `[inaudible]`, and `(applause)`.
- **Merge same-speaker turns** — joins consecutive `Name:` / `[Name]:` / `>> Name:` lines from the
  same speaker.
- **Fix capitalization / punctuation** — capitalizes sentence starts and `I`, spaces punctuation,
  and adds a final period when a line ends on a word.

### Limits

- This is not a grammar rewrite or summarizer. It cleans transcript scaffolding; it does not change
  sentence order, rewrite awkward phrasing, or infer missing punctuation from audio.
- Aggressive filler removal is opt-in because words such as `like`, `right`, and `actually` can be
  real content.
- Speaker detection is line-based and expects a leading label such as `Alice:` or `[Alice]:`.
- Bracket removal is broad: if you need to keep editorial notes in brackets, turn it off.

## FAQ

<details>
<summary>Does this use an AI model to rewrite my transcript?</summary>

No. The cleaner is deterministic rule-based code: fixed filler lists, timestamp patterns, speaker
label parsing, and punctuation spacing. That keeps it local and repeatable, but it will not rewrite
sentences the way a model would.

</details>

<details>
<summary>What is the difference between standard and aggressive filler removal?</summary>

`standard` removes only clear vocal pauses such as `um`, `uh`, `erm`, and `hmm`. `aggressive` also
removes discourse markers such as `you know`, `I mean`, `basically`, `actually`, and `like`. Use
aggressive for rough cleanup, but review the result because those words can be meaningful.

</details>

<details>
<summary>Can it clean SRT or VTT captions?</summary>

Yes. With **Remove timestamps** on, it drops SRT sequence numbers, cue timing lines containing
`-->`, the `WEBVTT` header, bracketed timestamps, and leading clock stamps before cleaning the
remaining caption text.

</details>

<details>
<summary>Why did a bracketed note disappear?</summary>

The **Remove bracketed cues** option strips both square-bracketed and parenthesized spans because
transcripts commonly use them for `[laughter]`, `[inaudible]`, and `(applause)`. Turn that option
off if your transcript uses brackets for content you want to keep.

</details>