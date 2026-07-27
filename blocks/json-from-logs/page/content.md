## About this tool

Logs and console dumps are mostly unstructured text, but the useful part is often a bit of
**JSON hiding inside a line** — a `state={…}` fragment, a serialized request/response, an array
of ids. Copying those out by hand and reformatting them is tedious and error-prone. Paste the
raw text here and this tool finds **every embedded JSON object or array**, validates each one,
and pretty-prints them as separate blocks — right in your browser.

How it works:

- It walks the text and, at every `{` or `[`, **brace-matches** a balanced run. The matcher is
  string- and escape-aware, so a `}` or `]` **inside a JSON string** never ends the run early.
- Each candidate run is **validated with a strict JSON parser**. Only runs that actually parse
  are kept, so a stray `{` in prose (or a `{…]` mismatch) is silently skipped and scanning
  resumes.
- Matched blocks are skipped over, so a **nested** object inside a larger one isn't extracted a
  second time — you get the top-level values, not every sub-object.
- **Key order is preserved** exactly as it appeared in the input.

Everything runs locally via WebAssembly — your logs never leave the page.

### Worked example

Input (a few log lines with JSON embedded in two of them):

```text
2026-07-24 10:12:01 INFO  starting worker
2026-07-24 10:12:02 DEBUG state={"user":"gizza","retries":0,"ok":true}
2026-07-24 10:12:03 INFO  batch ids [1, 2, 3] queued
```

Output (each block pretty-printed under a `// block N (line L)` header):

```text
// block 1 (line 2)
{
  "user": "gizza",
  "retries": 0,
  "ok": true
}

// block 2 (line 3)
[
  1,
  2,
  3
]
```

The `(line L)` tag is the line the block **started** on, so you can jump back to the original log
entry. Switch **Output shape** to `array` and the same input collapses into a single JSON array
of all extracted values — handy for piping into another tool.

### Options

- **Indent** — spaces per level for each block, `0`–`8`. Use `0` to **minify** each block to one
  compact line. Default `2`.
- **Output shape** — `blocks` (default) prints each extracted block separately under a
  `// block N (line L)` header, blank-line separated and line-numbered for triage; `array` wraps
  every extracted block into one pretty-printed JSON array for machine reuse.

### Limits

- **Strict JSON only.** Candidates are validated with a strict parser: trailing commas, comments,
  single quotes, unquoted keys, and other JSON5/JSONC relaxations are **rejected and skipped**,
  not repaired. Fix broken JSON first if you need those recovered.
- **Balanced brackets required.** A JSON value whose closing `}`/`]` is cut off (a truncated log
  line) won't match and is skipped.
- **Top-level values only.** Nested objects/arrays inside an extracted block are kept in place,
  not pulled out as their own blocks.
- **Max input 2 MB.** Larger pastes are rejected with a clear message; split the log first.
- If no valid JSON is found anywhere, you get a **"no JSON objects or arrays found"** message
  rather than empty output.

## FAQ

<details>
<summary>How does it tell real JSON from a stray brace in a log line?</summary>

Every `{`/`[` starts a candidate. The tool brace-matches a balanced run and then **parses it with
a strict JSON validator**. Only runs that parse successfully are kept, so a log line like
`shutting {down}` or a smiley `:-{` is matched, fails to parse, and is discarded — scanning simply
resumes one character later. You never see false positives dressed up as JSON.

</details>

<details>
<summary>What about braces or brackets inside JSON string values?</summary>

They're handled. The matcher tracks whether it's inside a `"…"` string (respecting `\"` escapes),
so a value like `{"text":"a } b ] c"}` matches as one complete object — the `}` and `]` inside the
quotes don't change the nesting depth. This is the main reason a naïve "count the braces" approach
fails on real logs and this one doesn't.

</details>

<details>
<summary>Does it extract nested JSON as separate blocks?</summary>

No. Once a top-level object or array is matched, the tool skips past it, so a nested object like
the inner `{"b":1}` in `{"a":{"b":1}}` stays **inside** its parent block rather than being reported
again on its own. You get one block per top-level embedded value, which is almost always what log
triage wants.

</details>

<details>
<summary>What's the difference between the "blocks" and "array" output shapes?</summary>

**`blocks`** (the default) is for reading: each extracted value is pretty-printed separately under
a `// block N (line L)` header, blank-line separated, with the line number it came from — ideal for
skimming a log dump. **`array`** is for reuse: every extracted value is wrapped into a single
JSON array (e.g. `[ {...}, {...} ]`) that is itself valid JSON you can paste into another tool or
save to a file.

</details>

<details>
<summary>Will it fix or repair malformed JSON?</summary>

No — validation is strict and deliberately so. Anything with a trailing comma, comment, single
quotes, or unquoted keys is treated as "not JSON" and skipped, so you never get a
plausible-but-wrong result. If your logs contain nearly-JSON that needs repairing first, run it
through a dedicated JSON-repair step and then extract.

</details>

<details>
<summary>Is there a size limit, and is anything uploaded?</summary>

The input is capped at **2 MB** — enough for large pasted console dumps — and oversized input is
rejected with a clear message so you can split the log. Nothing is uploaded: the whole scan runs
locally in your browser via WebAssembly, so even sensitive logs stay on your machine.

</details>
