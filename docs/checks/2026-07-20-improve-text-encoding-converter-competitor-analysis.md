# text-encoding-converter — competitor analysis (2026-07-20)

Build-time scan (before implementation). Backlog row: "Detects the character encoding of
pasted text or an uploaded file and converts between UTF-8, Shift-JIS, EUC-JP, GBK, Big5,
Latin-1, and more." Classified **pure Rust, Input::File (url⊕ref), chat+CLI, no page** —
same surface class as detect-file-type / document-skew-detector / bzip2-compress (a
byte-level file tool: the page generator has no file-upload runtime for pure blocks).

## Not a duplicate of charset-transcode

`blocks/charset-transcode` is a **mojibake fixer over pasted UTF-8 text**: it takes an
already-valid UTF-8 string, encodes its *characters* back to legacy bytes, and re-decodes
those bytes as UTF-8 (inverse transcoding); its "auto" mode is a repair heuristic, and its
only output is UTF-8 text. This tool is the **iconv/chardet byte-level converter**: raw
file bytes in an *unknown* encoding → statistical detection (chardetng, the Firefox
detector) + BOM sniffing → decode → re-encode to a chosen target, **including encoding TO
legacy charsets and UTF-16 (bytes out)**, which charset-transcode cannot represent at all.
Distinct input surface (raw bytes vs UTF-8 text), distinct engine (byte-level statistical
detection vs mojibake inversion), distinct output (bytes in any target encoding vs UTF-8
text). Page copy in both cross-references the other.

## Competitors skimmed (top real tools; one replaced)

1. **Localizely Text Encoding Converter** — file upload (drag/drop); 50+ encodings
   (ISO-8859-1..16, Windows-125x, DOS/Mac code pages, KOI8, GBK/GB2312/GB18030/Big5,
   Shift-JIS/EUC-JP, EUC-KR, UTF-8/16/32 incl. endianness); source select defaults
   **Auto-Detect** (with a caveat that detection is imperfect; ~21 detectable), target
   defaults **UTF-8**; single Convert button; in-browser processing.
2. **LDDGO File Encoding Detect and Convert** — file upload (1 MB free tier); separate
   **Detect** and **Convert** actions; detect lists candidate encodings **with confidence
   indices (1-100)**; detection set incl. UTF-8/16/32 (both endiannesses), Shift_JIS,
   ISO-2022-JP/CN/KR, GB18030, EUC-JP/KR, Big5, ISO-8859-x, windows-1251/1256, KOI8-R;
   conversion set 80+; From/To selectors; converted file downloads.
3. **CoderTools Charset Converter** — text mode AND file mode; 30+ encodings (UTF-8,
   UTF-16 LE/BE, GBK/GB2312/GB18030, Big5, Shift_JIS, EUC-JP, ISO-2022-JP, EUC-KR,
   ISO-8859-1/15, Windows-1252/1251, KOI8-R, ISO-8859-5, ASCII, Macintosh); auto-detect
   with manual override; **BOM add/remove controls**; unmappable characters "replaced
   with '?' or similar"; UTF-8 default target; preview + copy + download; also offers
   Base64/Hex/C-array in/out formats and a Show Hex toggle.
4. **SubExtractor Convert to UTF-8** — subtitle-focused; upload only; detects dozens of
   encodings (Western, GBK/GB2312/Big5, Shift-JIS/EUC-JP/ISO-2022-JP, EUC-KR/CP949,
   Cyrillic) with fallback; fixed UTF-8 output; 5 MB text limit.

(encoding-converter.com was in the search results but returned HTTP 403 → replaced with
CoderTools + SubExtractor per the don't-run-with-fewer rule.)

## Table stakes → in-model / out-of-model

| Capability | Tag | Disposition |
|---|---|---|
| File upload input | in-model | `Input::File` url⊕ref (chat/CLI). No page: pure blocks have no page file-upload runtime (platform), like detect-file-type. |
| Source encoding with **auto-detect default** | in-model | `from` string param, default `auto`; BOM sniff (UTF-8/16/32 LE+BE) first, then chardetng; accepts any WHATWG label for manual override. |
| Target encoding, **default UTF-8** | in-model | `to` string param, default `utf-8`; any WHATWG label with an encoder + hand-rolled UTF-16LE/BE writers (encoding_rs has no UTF-16 encoder). |
| Broad charset coverage (ISO-8859-x, Windows-125x, GBK/GB18030/Big5, Shift_JIS/EUC-JP/ISO-2022-JP, EUC-KR, KOI8, Mac) | in-model | encoding_rs WHATWG label set (proven in charset-transcode) covers all of these for decode AND encode. |
| UTF-16/UTF-32 input incl. endianness | in-model | UTF-16LE/BE via encoding_rs decoders; UTF-32LE/BE hand-rolled (BOM-detected or explicit `from`). |
| Separate Detect action with a report (LDDGO) | in-model | `mode` enum `convert\|detect` (default convert); detect returns JSON: detected name, how (bom/detector/ascii), BOM, validity, candidate list, text preview. |
| Detection **confidence score** (LDDGO 1-100) | out-of-model | chardetng exposes a single best guess, no numeric score; faking one would be dishonest. Compensated with a clean-decode `candidates` list + `note`. |
| **BOM add/remove** (CoderTools) | in-model | `bom` boolean (default false) for UTF-8 output; UTF-16 output always BOM'd (standard practice, documented); `bom=true` with a legacy target errors. Input BOMs are always stripped on convert. |
| Unmappable-char handling ('?' replace) | in-model | `errors` enum `replace\|strict` (default replace): replace substitutes `?` on encode / U+FFFD on decode and reports counts; strict fails with position. |
| Decoded-text preview | in-model | first ~160 chars in detect JSON and in the convert `for_llm` summary. |
| Converted file download | in-model | media envelope `data:` URL (`text/plain; charset=<to>`); CLI saves the file. |
| Pasted-TEXT mode (CoderTools text mode) | out-of-model here | pasted text is already UTF-8, so byte-level detection degenerates; that surface is `charset-transcode`'s (cross-referenced) — duplicating it would re-ship that block. |
| Batch / multiple files | out-of-model | single upload slot per call (platform); call per file. |
| Base64/Hex/C-array in/out, Show Hex (CoderTools) | out-of-model | separate existing blocks (base-decoder, multi-encoder, hex tools); scope creep here. |
| 1–50 MB size tiers | in-model (bounded) | 8 MiB cap — above LDDGO's free 1 MB and SubExtractor's 5 MB; bounded for the 64 MiB wasm sandbox (bytes + decoded string + encoded output + base64 envelope must coexist). |

Every table-stake is either in the descriptor or in the out-of-model list above — none
dropped silently. No competitor copy, branding, or trademarks were reused; behaviors were
paraphrased from feature lists only.
