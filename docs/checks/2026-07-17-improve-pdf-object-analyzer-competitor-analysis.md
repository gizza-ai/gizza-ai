# pdf-object-analyzer — competitor analysis (2026-07-17)

## What the tool does
Statically walks a PDF's indirect-object tree and surfaces the structural
indicators used in malicious-document triage: automatic actions that fire on
open (`/OpenAction`, `/AA`), embedded JavaScript (`/JS`, `/JavaScript`) with the
source snippet extracted, `/Launch` actions that can start an external program
(with the target), embedded files (`/EmbeddedFile`, `/Filespec`) with their
names, object streams (`/ObjStm`) and other obfuscation/attack-surface features
(`/XFA`, `/AcroForm`, `/JBIG2Decode`, `/RichMedia`), and outbound links
(`/URI`, `/SubmitForm`, `/GoToR`). It reports the PDF version, object / stream /
page / object-stream counts, encryption, and every indicator with its category,
count, and the object ids it appeared in, then assigns a coarse risk level
(`none` / `low` / `medium` / `high`) with plain-language reasons.

Pure-Rust: `lopdf` (default-features off) is the PDF engine dep. The runtime is
network-free: callers pass the PDF bytes as `pdf_base64` (a
`data:application/pdf;base64,...` prefix is accepted) plus optional `detail`.
Surfaces: **chat + CLI**. This is *static structural* analysis: it never
executes, sandboxes, or renders the PDF, and it does **not** return a malware
verdict.

## Top competitors

1. **PDFiD** (Didier Stevens) — the canonical triage scanner: counts a fixed set
   of suspicious names (`/JS`, `/JavaScript`, `/OpenAction`, `/AA`, `/Launch`,
   `/EmbeddedFile`, `/AcroForm`, `/JBIG2Decode`, …) and flags the ones present.
   Counts only; it does not extract the JavaScript or the launch/URL targets.
2. **pdf-parser.py** (Didier Stevens) — walks the object tree, resolves
   references, filters by type/keyword, and dumps/decompresses individual stream
   contents. Powerful but manual and command-driven; no single risk summary.
3. **peepdf / peepdf-3** — interactive analysis console that maps the object
   tree, isolates JavaScript, decodes streams, and highlights obfuscation and
   known-vulnerable elements; can shell out to a JS engine for emulation.
4. **PDFStreamDumper** — Windows GUI that decodes streams, extracts embedded
   JavaScript, and scans for known exploit shellcode signatures.
5. **VirusTotal / Hybrid Analysis (online)** — upload-and-scan services that run
   multi-engine AV, YARA, and behavioural detonation, returning a
   maliciousness verdict and a community reputation score.

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza tool | Verdict |
|---|---|---|---|
| Suspicious-name keyword scan + counts | yes (PDFiD) | **yes** (per-indicator key, category, count) | parity |
| Object / stream / page counts, PDF version | partial | **yes** | parity |
| `/OpenAction` + `/AA` auto-run detection | yes | **yes** (+ resolves the `/OpenAction` action kind) | parity / edge |
| Extract embedded JavaScript source | partial (pdf-parser, peepdf) | **yes** (literal + decompressed stream, capped snippet) | parity |
| `/Launch` target extraction | rare | **yes** (bare `/F`, file-spec `/F`, Windows `/Win`) | edge |
| Embedded-file names | partial | **yes** (`/UF`⊕`/F` from `/Filespec`) | parity |
| Referenced URLs (`/URI`) | partial | **yes** | parity |
| Object-stream (`/ObjStm`) + obfuscation flags | yes (PDFiD) | **yes** (`/ObjStm`, `/XFA`, `/JBIG2Decode`, `/RichMedia`) | parity |
| Per-indicator object-id provenance | partial (pdf-parser) | **yes** (object ids behind each hit, capped) | edge |
| Coarse risk level + reasons | rare | **yes** (`none`/`low`/`medium`/`high` with reasons) | edge |
| Encryption flag | yes | **yes** (`/Encrypt` in trailer) | parity |
| **Dump/decompress arbitrary streams to disk** | yes (pdf-parser, PDFStreamDumper) | **no** | out-of-model gap |
| **JavaScript deobfuscation / emulation** | yes (peepdf) | **no** | out-of-model gap |
| **Known-exploit / CVE / shellcode signatures** | yes (PDFStreamDumper) | **no** | out-of-model gap |
| **AV / YARA verdict + behavioural detonation** | yes (VT, Hybrid Analysis) | **no** | out-of-model gap |

## Table-stakes coverage
The triage table stakes set by PDFiD — flag `/JS`, `/JavaScript`, `/OpenAction`,
`/AA`, `/Launch`, `/EmbeddedFile`, `/AcroForm`, `/JBIG2Decode`, `/ObjStm`, and
report counts + encryption — are all met, and the tool goes past counts by
extracting the JavaScript source, the `/Launch` and embedded-file targets, and
the referenced URLs, attaching object-id provenance to every indicator, and
rolling the whole picture into a coarse risk level with reasons. The `detail`
parameter (`full` default / `summary`) lets a caller drop the bulky evidence
(JS source + object-id lists) when only the structural overview is needed.
The PDF input is base64 rather than URL/ref so the analyzer remains pure and
does not depend on `wafer-run/network` capabilities.

## Out-of-model features (intentionally NOT built)
- **Stream dumping / decompression to files** — pdf-parser and PDFStreamDumper
  let an analyst extract and save any stream. This tool decompresses JavaScript
  streams for the snippet but is a read-only reporter, not a stream extractor;
  full dumping is a different, stateful surface.
- **JavaScript deobfuscation / emulation** — peepdf can drive a JS engine to
  unravel obfuscated payloads. That needs a sandboxed interpreter and is far
  outside a stateless, non-executing analyzer.
- **Exploit / CVE / shellcode signature matching** — requires a maintained
  signature database and is a detection product in its own right.
- **AV/YARA scanning + behavioural detonation** (VirusTotal, Hybrid Analysis) —
  needs multi-engine infrastructure and a sandbox; this tool deliberately makes
  no malware verdict and says so.

These are listed, not built, per the improve-tool rule (no competitor
copy/branding reused).

## Page / Playwright: not applicable
This is a base64-PDF → structured-JSON forensic triage tool, so it ships as
**chat + CLI only** rather than a generic static page. There is therefore no
tool page and no Playwright spec; the descriptor/schema drift-guard test
validates what the chat surface consumes, and the CLI exact-output check
exercises the runtime path.

## Verification (this build)
- `CARGO_BUILD_JOBS=1 cargo test --workspace`: block + core tests pass
  (incl. the drift-guard schema test, benign/malicious/summarized/parse-error
  cases).
- `wafer build` from `blocks/pdf-object-analyzer/`: block builds, copies, and
  validates under wasm32-wasip1 — ~881 KiB `block.wasm`.
- CLI (`gizza tool pdf-object-analyzer pdf_base64=…`): a generated auto-run-JavaScript +
  `/Launch` + embedded-file PDF → `risk_level: "high"` with the indicators, JS
  snippet, launch/embedded targets; a benign PDF → `risk_level: "none"` with no
  indicators.
- No page surface (binary file input → JSON), as documented; no Playwright spec.

Sources: blog.didierstevens.com (PDFiD, pdf-parser), github.com/jesparza/peepdf
(peepdf-3), sandsprite.com (PDFStreamDumper), virustotal.com,
hybrid-analysis.com.
