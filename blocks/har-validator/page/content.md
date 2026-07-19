## About this tool

**HAR Validator** checks a **HAR (HTTP Archive)** file against the
[HAR 1.2 specification](http://www.softwareishard.com/blog/har-12-spec/) and
tells you exactly what's wrong — or confirms it's clean.

- **Structure:** verifies the top-level `log` object and its required
  `version`, `creator`, and `entries`.
- **Every entry:** checks each entry's required `startedDateTime`, `time`,
  `request`, `response`, `cache`, and `timings`, plus the required fields inside
  `request` (`method`, `url`, `headers`, `cookies`, `queryString`, …) and
  `response` (`status`, `content.mimeType`, `redirectURL`, …).
- **Exact locations:** every problem is reported with its **JSON path**
  (e.g. `log.entries[3].response: missing required field "content"`) and whether
  a field was *missing* or the *wrong type* — and it lists **all** problems at
  once, not just the first.
- **Timing totals:** with *Check timing totals* on, each entry's `time` is
  compared to the sum of its timing phases (`blocked + dns + connect + send +
  wait + receive`, with `-1` phases excluded). Mismatches are reported as
  **warnings** — they don't make the file invalid, but they often reveal a
  broken exporter.

A well-formed but incomplete HAR still parses as JSON, so a plain formatter
won't catch a missing `timings` block — this tool does.

Everything runs **locally in your browser** via WebAssembly — your capture,
which can contain URLs, cookies, and headers, is never uploaded.

### Handy for

- Debugging a HAR your own tool or script generates before feeding it to an
  analyzer.
- Confirming a browser DevTools / Charles / Fiddler export is spec-complete.
- Spotting exporters that write inconsistent `time` vs. phase timings.

## FAQ

<details>
<summary>What is a HAR file?</summary>

A **HAR (HTTP Archive)** file is a JSON log of a browser's network activity —
every request and response, with headers, cookies, timings, and sizes. You can
export one from Chrome/Firefox DevTools (Network tab → *Save all as HAR*) or
proxies like Charles and Fiddler. This tool checks that the JSON matches the
HAR 1.2 shape those tools are supposed to produce.

</details>

<details>
<summary>What does "valid" mean here — and what's a warning vs. an error?</summary>

An **error** is a structural spec violation: a missing required field or one
with the wrong JSON type. Any error marks the file **invalid**. A **warning**
is a consistency issue that doesn't break the structure — currently a mismatch
between an entry's `time` and the sum of its timing phases. A file with only
warnings is still **valid** HAR.

</details>

<details>
<summary>Why does timing-total checking say my times don't add up?</summary>

The HAR spec says an entry's `time` should equal the sum of its non-negative
timing phases (`blocked + dns + connect + send + wait + receive`; `ssl` is
counted inside `connect`, and a phase of `-1` means "not applicable" and is
excluded). Some exporters round each phase independently or track total time
separately, so small drift is common — differences under **1 ms** are ignored.
Larger gaps usually mean the exporter computed `time` a different way. Turn the
check off if you only care about structure.

</details>

<details>
<summary>Is my HAR uploaded anywhere?</summary>

No. Validation runs entirely in your browser through WebAssembly — the HAR
never leaves your machine. That matters because captures routinely contain
session cookies, auth headers, and full URLs.

</details>

<details>
<summary>Which HAR versions does it accept?</summary>

It validates against the **HAR 1.2** field set, which is a superset of 1.1, so
1.1 files pass too. The reported version comes from your file's `log.version`;
the required-field checks are the same across 1.x. Vendor-specific extension
fields (anything beyond the spec) are simply ignored, not flagged.

</details>
