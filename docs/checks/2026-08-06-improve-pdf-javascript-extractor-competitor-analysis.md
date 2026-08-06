# pdf-javascript-extractor — competitor analysis (2026-08-06)

Scan run BEFORE implementing, per `/improve-tool` Phase 2–3. One web search
("extract embedded JavaScript from PDF deobfuscate analysis tool") plus targeted
reads of the three most-cited reference tools. **Everything below is paraphrased
from public documentation — no competitor copy, branding, or trademarks were
copied.** Names are referenced only to identify the tool being described.

## Tools reviewed

### 1. Didier Stevens' PDF tools (`pdfid.py` + `pdf-parser.py`) — CLI, Python
The de-facto triage pair. `pdfid` counts risky keywords (`/JS`, `/JavaScript`,
`/OpenAction`, `/AA`, `/Launch`, `/EmbeddedFile`) without parsing the object
graph; `pdf-parser` then walks objects and can select and dump one. Relevant
option shapes: pick an object by number, search by keyword/name, filter stream
content through the declared PDF filters (FlateDecode etc.) or dump it raw, and
print structural statistics. Output is a per-object text dump — the analyst is
expected to pipe the extracted script into a separate beautifier.
*Takeaway:* keyword-driven location + "decoded stream vs raw stream" is the
baseline. De-obfuscation of the JavaScript itself is explicitly out of scope
there, which is exactly the gap this tool fills.

### 2. peepdf — interactive CLI, Python
Parses the physical and logical structure, tracks object references, and offers
extraction shortcuts for objects, JavaScript, and shellcode. Documents decoding
of hexadecimal and octal PDF encodings and the common stream filters, plus
JavaScript-side helpers described as unescape / replace / join, and a
beautifier. Full JavaScript analysis is optional and depends on an embedded
JS engine (PyV8 historically) — the project's own notes list better automatic
JavaScript analysis as unfinished work.
*Takeaway:* per-script location + reference tracking, string-level de-obfuscation
(unescape/replace/join), and beautify are all table stakes. The engine-backed
dynamic evaluation is not.

### 3. PDF Stream Dumper — Windows GUI
Bundles many analysis utilities behind one UI: browse every object/stream,
decode stream contents, and de-obfuscate JavaScript, with shellcode emulation
and scan-for-known-exploit features alongside.
*Takeaway:* the "one pass gives you every script, decoded and readable" UX is the
expectation; emulation is a separate, heavier feature class.

### Cross-checked analyst workflow (practitioner write-up)
The manual flow analysts actually run: find the object holding the script,
inflate the stream, beautify, then unwind the encoding layers — most commonly
`String.fromCharCode` over a decimal/hex array feeding `eval`, `unescape` over
`%XX`/`%uXXXX` blobs, backslash `\xNN`/`\uNNNN`/octal escapes inside literals,
base64 via `atob`, and string-splitting/concatenation used to hide identifiers.
The payoff at the end is IOCs — URLs and the Acrobat API names that pin the
exploit family.

## Table-stakes list (each tagged in-model / out-of-model)

| Capability | Verdict | Where it landed |
|---|---|---|
| Find scripts in the document-level `/Names → /JavaScript` name tree (incl. nested `/Kids`), with the entry name | **in-model** | `location`, `trigger = document-level` |
| Find `/OpenAction` scripts | **in-model** | `trigger = document-open` |
| Find `/AA` additional-action scripts (catalog, page, annotation, form field) with the event key | **in-model** | `trigger` + `location` |
| Find annotation `/A` action scripts and `/Next` action chains | **in-model** | walked |
| Catch-all sweep for any other `/JS` dictionary | **in-model** | `trigger = object-scan` |
| Inflate `FlateDecode` (and other declared filters) stream-held scripts | **in-model** | `source_kind = stream`, via lopdf |
| Decode PDF text strings (UTF-16BE BOM / PDFDocEncoding, hex strings) | **in-model** | `source_kind = string` |
| Report per-script object id, byte length, trigger, location | **in-model** | `Script` fields |
| De-obfuscate `String.fromCharCode(…)` (decimal + `0x` hex) | **in-model** | `decodings: from-char-code` |
| De-obfuscate `unescape()` / `decodeURIComponent()` over `%XX` and `%uXXXX` | **in-model** | `decodings: percent-unescape` |
| De-obfuscate `atob()` base64 blobs | **in-model** | `decodings: base64-atob` |
| Decode `\xNN` / `\uNNNN` / octal escapes inside string literals | **in-model** | `decodings: string-escapes` |
| Fold `"a" + "b"` literal concatenation used to split identifiers | **in-model** | `decodings: string-concat` |
| Iterate the passes so nested layers unwrap | **in-model** | up to 4 rounds, `rounds` reported |
| Beautify / re-indent the result | **in-model** | `beautify` param, reuses the `js-beautify` core |
| Keep the untouched original available for comparison | **in-model** | `include_raw` param |
| Flag suspicious Acrobat/JS API names (eval, `app.launchURL`, `util.printf`, `Collab.collectEmailInfo`, `media.newPlayer`, `exportDataObject`, `spell.customDictionaryOpen`, `getAnnots`, `app.setTimeOut`, `ActiveXObject`/`WScript.Shell`, …) | **in-model** | `indicators` |
| Pull URL/IOC strings out of the decoded source | **in-model** | `urls` |
| Summary vs full report depth | **in-model** | `detail` enum |
| Per-script output cap so a huge script can't blow the response | **in-model** | `max_script_chars` |
| Execute/emulate the JavaScript in a JS engine (PyV8/SpiderMonkey) | **out-of-model** | gizza is pure-Rust wasm, no JS engine; static only |
| Shellcode emulation / libemu | **out-of-model** | needs an x86 emulator |
| VirusTotal / reputation lookups | **out-of-model** | no accounts, no API keys, no backend |
| Interactive object browser GUI, PDF modification/repair | **out-of-model** | different tool shape (see `pdf-object-analyzer` for structure mapping) |
| Decrypting an encrypted PDF to reach its scripts | **out-of-model (stated limit)** | reported via `encrypted` + a note on the page/description |

## Defaults chosen (and why)

- `deobfuscate = true` — every reference workflow's first move after extraction;
  the raw form stays reachable via `include_raw`.
- `beautify = true` — obfuscated PDF JavaScript is usually one long line; all
  three tools ship or recommend a beautifier.
- `detail = full` — matches the sibling `pdf-object-analyzer` default, and the
  script source is the point of this tool.
- `max_script_chars = 20000` — comfortably above real-world droppers (a few KB)
  while bounding the response; `truncated` is reported per script and overall.
- 16 MiB input cap — same as `pdf-object-analyzer`, and well inside the wasm
  sandbox's memory budget.

## Worked example carried into the description/FAQ

A one-page PDF whose `/OpenAction` runs
`eval(unescape("%61%70%70%2e%61%6c%65%72%74%28%31%29"))` reports one script at
`/OpenAction` with `trigger = document-open`, `decodings = ["percent-unescape"]`,
and the decoded source `eval("app.alert(1)")`, plus an `eval` indicator.

## Not a duplicate of `pdf-object-analyzer`

`pdf-object-analyzer` maps the whole object tree and assigns a coarse risk level;
its JavaScript output is a de-duplicated list of 800-character snippets with no
decoding, no location, and no per-script metadata. This tool does the opposite:
it ignores non-script structure and goes deep on the scripts — every location and
trigger, full source up to the cap, layered de-obfuscation, beautification, JS-API
indicators, and IOCs. Confirmed by reading
`blocks/pdf-object-analyzer/core/src/lib.rs` (`snippet()` caps at 800 chars,
`push_unique` de-dupes and drops location).

## Sources

- <https://zeltser.com/tools-for-malicious-pdf-analysis>
- <https://github.com/jesparza/peepdf>
- <https://tho-le.medium.com/pdf-forensics-javascript-analysis-part-2-b177f5d5d579>
