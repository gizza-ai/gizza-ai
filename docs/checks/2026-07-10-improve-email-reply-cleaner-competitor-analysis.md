# email-reply-cleaner — competitor analysis (2026-07-10)

Snapshot taken while building the tool (new-tool + improve-tool procedure). Goal: strip
`>` quote markers, quoted reply chains, and signature/footer blocks from replied/forwarded
plain-text email so only the fresh message remains. All findings paraphrased — no competitor
copy, branding, or trademarks reproduced.

## Competitors scanned

1. **github/email_reply_parser** (Ruby, the original) — splits a plain-text body into
   fragments (visible / quoted / signature / hidden). Detects quoted text by lines beginning
   with `>` and an `On … wrote:` attribution line; signatures by a line starting `-- ` /
   underscores. Public API: `parse_reply(body)` returns the visible text. Documented limits:
   English-only ("on"/"wrote"), multi-line wrapped attribution headers not caught.
2. **crisp-oss/email-reply-parser** (Node, ~1M inbound emails/day) — strips
   `On DATE, NAME <EMAIL> wrote:` attributions, quote markers, signatures (incl. mobile
   footers like "Sent from my iPhone") and farewell lines. Ships ~10 locales (EN/FR/ES/PT/
   IT/JA/ZH …). API: `.read(text).getVisibleText()`; optional RE2 engine.
3. **cleancopiedtext.com — Clean Email Thread** — consumer, browser-local. Removes: quoted
   lines beginning with `>`, header lines (`From:` / `Sent:` / `To:` / `Subject:`), reply
   attributions ("On [date], [person] wrote:"), and signature blocks after the `-- ` delimiter.
   UX: paste ↔ result panels, **"Fine-tune options let you pick exactly which transformations
   to apply"**, sample button, char/word/line counts, Copy result, Download .txt, "Use as
   input" for multi-pass.
4. **CleanMyText — Remove Email Indents** — single-purpose: strip the `>` quote-prefix markers
   so replied text reads cleanly. Minimal UI, in-browser.
5. **Mailgun Talon** (Python, server) — most sophisticated: heuristics + a **machine-learning**
   classifier for signature lines and quote detection across many formats.

## Table-stakes → decision

| Capability | In/Out of model | Decision |
| --- | --- | --- |
| Remove `>` (and nested `>>`) quote-prefix lines | in-model | `remove_quotes` param (default on) |
| Cut `On … wrote:` attribution + everything below (the reply chain) | in-model | `remove_reply_chain` param (default on) |
| Cut Outlook `From:/Sent:/To:/Subject:` header blocks & `-----Original Message-----` | in-model | folded into `remove_reply_chain` |
| Cut `---------- Forwarded message ----------` / `Begin forwarded message:` | in-model | folded into `remove_reply_chain` |
| Cut signature after the `-- ` (RFC 3676) delimiter | in-model | `remove_signature` param (default on) |
| Cut mobile/app footers ("Sent from my iPhone", "Get Outlook for …") | in-model | folded into `remove_signature` |
| Collapse runs of blank lines / trim edges | in-model | `collapse_blank_lines` param (default on) |
| Per-transformation fine-tune toggles | in-model | the four boolean params above |
| Copy result / Download .txt | in-model (platform) | shared page provides Copy + Download on `format="text"` |

## Considered, not built (out-of-model or platform-declined)

- **ML signature classification (Talon).** Needs a trained model / server; gizza is pure-Rust
  wasm. Our signature detection is heuristic (the `-- ` delimiter + a curated footer list) and
  said so on the page.
- **Full ~10-locale attribution (Crisp).** We detect the dominant **English** attribution
  ("On … wrote:") plus the language-neutral structural markers (`>` prefixes, `-- ` delimiter,
  `From:`/`Sent:` headers, forwarded-message rules). Broader localisation is listed as a limit
  on the page, not silently dropped.
- **Multi-pass "Use as input" chaining / live word-count panel (cleancopiedtext).** The shared
  gizza page already offers Copy result and a Download link for `format="text"`; the transform
  is a single deterministic pass, so a chaining loop adds no capability. Declined.
- **Multi-line wrapped attribution headers** (Gmail's 80-col wrap of `On … wrote:`). Same
  limitation the original github lib documents; we detect single-line attributions and state
  the limit on the page.

## Our design (all in-model table-stakes covered)

Params: `text` (required, multiline) · `remove_quotes` (bool, default true) ·
`remove_reply_chain` (bool, default true) · `remove_signature` (bool, default true) ·
`collapse_blank_lines` (bool, default true). Pure Rust, browser-local, no network, no model.
