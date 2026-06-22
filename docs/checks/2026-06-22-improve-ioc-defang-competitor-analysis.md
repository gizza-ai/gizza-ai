# ioc-defang — competitor analysis (2026-06-22)

Tool: `blocks/ioc-defang` — defang/refang IOCs (URLs, IPs, domains, emails).
Surfaces verified: chat (drift-guarded schema), CLI (`gizza tool ioc-defang`),
page (`/tools/ioc-defang/`, Playwright green).

## Top competitors surveyed

1. **APIVoid URL Defanger** (apivoid.com/tools/url-defang) — defang + refang URLs,
   IPs, domains. `http`→`hxxp`, `.`→`[.]`. Square brackets only. No bracket-style choice.
2. **InventiveHQ URL Defanger** (inventivehq.com/tools/security/url-defanger) — defang +
   auto-refang URLs/IPs/domains. Square brackets.
3. **VariedTools Defang/Refang** (variedtools.com/defang-refang-url) — defang/refang URLs,
   domains, IPv4, email. Bidirectional.
4. **CyberChef** "Defang URL" / "Defang IP Address" + "Refang URL" operations — toggles for
   escaping dots, http→hxxp, `://`→`[://]`. Square brackets.
5. **InQuest `iocextract`** (PyPI / GitHub) — Python library; `refang_*` + `defang` plus IOC
   *extraction* from unstructured text (a different, larger scope).

## Feature diff vs. ioc-defang

| Capability | Competitors | ioc-defang | Status |
| --- | --- | --- | --- |
| Defang URL scheme (`http`→`hxxp`, `https`→`hxxps`, `ftp`→`fxp`) | yes | yes | matched |
| Defang `.` between labels | yes | yes | matched |
| Defang `@` in email | VariedTools, CyberChef | yes (`[at]`) | matched |
| Defang `://` → `[://]` | CyberChef | yes | matched |
| Refang (inverse) | APIVoid, InventiveHQ, VariedTools, CyberChef | yes | matched |
| Refang recognizes `()`/`{}`/`[dot]`/`meow://` | partial (most square-only) | yes | **ahead** |
| Bracket-style choice (square/round/curly/dot) | rare (most fixed square) | yes (4 styles) | **ahead** |
| Whole-paragraph (defang only the IOC chars, keep prose) | yes | yes | matched |
| Case-insensitive scheme (`HTTP://`) | partial | yes | matched |
| Works fully offline / no upload | varies (most are server-side) | yes (browser-local wasm) | **ahead** |

## Gaps considered and decisions

- **IOC extraction from free text** (pull IPs/hashes/URLs out of a blob, like iocextract /
  mlab.sh) — deliberately OUT of scope. That's a distinct tool (extract, not transform); a
  future `ioc-extract` block would cover it. Not a defang gap.
- **File-hash defanging** (MD5/SHA) — hashes have no clickable chars to neutralize, so
  competitors don't transform them either; nothing to do.
- **Per-IOC-type toggles** (defang only IPs, leave URLs) — competitors that offer this are
  marginal; the simple "neutralize all indicator chars in the text" model matches the dominant
  workflow (paste a report, get a safe blob) and the `style` choice already covers the real
  variation. Not added (keeps the API small + the drift schema clean).

## Conclusion

ioc-defang matches the core defang/refang feature set of every surveyed competitor and is
**ahead** on (a) four bracket styles vs. the usual fixed square, (b) a refanger that accepts
square/round/curly/`[dot]`/`[at]`/`meow://` variants, and (c) fully browser-local execution
(no upload of potentially-live malicious indicators). No in-model gaps remain open; IOC
*extraction* is a separate future tool, not a gap in this one. 16 core unit tests + drift guard
pass; CLI and page verified.
