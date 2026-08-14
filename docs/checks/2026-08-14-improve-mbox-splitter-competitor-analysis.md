# mbox-splitter — competitor analysis (2026-08-14)

Scan run **before** implementing, per `/create-next-tool` step 4. One WebSearch
("split mbox file into individual eml files online tool"); the top real tools that actually
do this job were reviewed. All notes are **paraphrased observations** of what the category
ships — no competitor copy, branding, or trademarks are reused anywhere in this tool.

## Competitors reviewed

| # | Tool | Shape | What it does |
|---|------|-------|--------------|
| 1 | Aspose email app — MBOX to EML | Server-side web app | Upload an mbox, splits it into one `.eml` per message keeping headers/body/attachments, returns a single ZIP download. |
| 2 | OnlinePCTools MBOX → EML converter | Browser-local web app | Splits on the standard `From ` postmark, parses headers + body, writes one `.eml` per message, download-all in one go; states nothing is uploaded. |
| 3 | Univik MBOX → EML converter | Web app | Positions on source archives (Thunderbird / Apple Mail / Gmail Takeout) → separate `.eml` files for sharing and archiving. |
| 4 | `mbox2eml` (jahendrie, C) + `mbox-to-eml` (Riadz, Python) | CLI scripts | The reference open-source behaviour: split on the `From ` separator line, write `NNNN.eml` (or subject/date-derived names) into an output directory; the postmark line is dropped. |
| 5 | Desktop converter wizards (Advik, Softaken, DotStella writeups) | Installed apps | Same split, plus batch folders, naming-pattern menus (subject / date / from / auto-increment), and preview lists. |

## Table stakes → decision

| Capability | Competitors | Ours | Where |
|---|---|---|---|
| Split on the classic `From ` postmark line at column 0 | all | **in-model — built** | `split_messages`, postmark distinguished from a `From:` header |
| Drop the postmark from each `.eml` (RFC 5322 output) | all | **in-model — built** (default) | `keep_postmark = false` default; opt back in for round-tripping |
| Preserve each message verbatim (headers, MIME parts, base64 attachments) | all | **in-model — built** | message bytes are sliced, never re-serialized |
| One suggested filename per message | all | **in-model — built** | `naming` = index / subject / date / message-id, always index-prefixed so order + uniqueness survive |
| Naming-pattern menu (subject, date, auto-increment) | 5 (desktop wizards) | **in-model — built** | `naming` enum, 4 choices |
| Extract just one message | 4, 5 | **in-model — built** | `message = N` (1-based); with `output = eml` this yields a clean single `.eml` the page can download |
| Preview / list of what's inside before converting | 2, 5 | **in-model — built** | `output = list` — numbered table with filename, date, from, subject, size |
| Structured output for scripting | 4 | **in-model — built** | `output = json` (metadata + raw message text per entry) |
| Local / private processing, no upload | 2 | **in-model — built** | pure Rust→WASM, runs in the browser page and in the CLI; nothing leaves the machine |
| mboxrd/mboxo `>From ` unquoting | 4 (script-level) | **in-model — built** | `unescape_from` (default on) restores body lines that the exporter escaped |
| Handles a lone `.eml` with no postmark | 4 | **in-model — built** | treated as a single message rather than an error |
| RFC 2047 encoded-word subjects decoded for filenames | 1, 5 | **in-model — built** | `mail-parser` decodes the subject/date used in names |

## Out of model (listed, not built)

- **ZIP of every `.eml` as one download.** The page runtime only renders `text`, `image`,
  `audio` or `video` output (`tools/generator/assets/runtime/tool.js`), so a multi-file ZIP has
  no page surface; `create-zip` covers zipping and is deliberately page-less for the same
  reason. Our substitute is honest and in-model: `output = files` prints every message with a
  `===== NNN-name.eml =====` header (copy/paste or download as one text file), and
  `output = eml` + `message = N` yields exactly one raw `.eml` — the page's Download link saves
  it (as `mbox-splitter-output.txt`; rename to `.eml`).
- **Direct folder/batch output to disk with the suggested filenames.** No filesystem surface in
  the block model; the CLI prints to stdout, so a shell redirect per message is the workaround
  documented on the page.
- **Attachment extraction into separate files.** Out of scope for a splitter; `eml-parse` already
  lists a message's attachments.
- **Upload of multi-GB archives.** Input arrives as text through the page field / CLI arg, so
  very large exports are outside this surface; the tool caps at 2000 messages and says so.

## Sibling tools checked for overlap (not duplicates)

- `mbox-dedup` — removes duplicate messages **and returns an mbox**; no per-message split/naming.
- `gmail-takeout-parser` — mbox → one **table row** per message (CSV/JSON metadata); no `.eml` output.
- `eml-parse` — parses **one** `.eml` into structured fields; the inverse direction.
- `document-splitter` / `file-splitter` — text/section splitting, mbox-unaware.

## UX decisions taken from the scan

- Preset chips for the three real jobs (split all to `.eml`, list what's inside, pull one message)
  because the desktop wizards front-load naming/preview modes.
- Friendly `[input.labels]` on `output` and `naming` so the modes read as tasks, not enum values.
- Multiline mbox field with a realistic two-message placeholder (postmark + headers + body).

## Sources

- https://products.aspose.app/email/conversion/mbox-to-eml
- https://www.onlinepctools.com/converter/mbox-to-eml
- https://univik.com/converter/mbox-to-eml.html
- https://github.com/jahendrie/mbox2eml
- https://github.com/Riadz/mbox-to-eml
