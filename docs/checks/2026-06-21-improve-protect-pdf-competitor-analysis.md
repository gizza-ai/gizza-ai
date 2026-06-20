# protect-pdf — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/protect-pdf` — add password encryption to a PDF using the PDF
standard security handler (AES-256). Pure-Rust (`lopdf`). Document(PDF) input →
PDF output, so **chat + CLI, no page** (like `pdf-form-fill` / `pdf-compress`).

## What competitors do

- **Online "protect PDF / add password" sites** (smallpdf, ilovepdf, sodapdf, …)
  — upload, set a password, download. Strength: easy. **Weakness: you upload the
  document you're trying to keep private to a third-party server**; free tiers cap
  files/day and some watermark.
- **`qpdf --encrypt`, `pdftk … output … user_pw`** — local + scriptable and
  correct, but require installing native tooling and learning the flags.
- **Acrobat / "Office → export with password"** — desktop apps, paid, not
  scriptable.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`lopdf`) compiled to wasm:
   runs in the chat Service Worker and headless in the CLI. The PDF and the
   password never leave the device — the whole point of protecting the file.
2. **Strong, modern encryption.** Uses the AES-256 standard security handler
   (encryption V5 / revision 6) with a random 256-bit file key — not the legacy
   RC4/40-bit encryption many quick tools still emit. Opens with the password in
   any compliant viewer (Acrobat, Preview, pdf.js, etc.).
3. **Separate owner password (optional).** Supply a distinct `owner_password` to
   control permissions independently of the open password; defaults to the open
   password when omitted.
4. **Chainable + agent-friendly.** Takes the PDF by `url` or `ref` and emits the
   protected PDF as a downloadable envelope (itself a `ref`), so it composes with
   the other PDF tools and is callable identically from chat and CLI.

## Honest scope

- **Open-password focused.** All standard permissions are granted (the goal is an
  open password, not a locked-down/restricted document); fine-grained permission
  flags are not exposed.
- **Refuses already-encrypted input** with a clear message rather than
  double-encrypting.
- **No page** — Document input + PDF-bytes output don't fit the page's
  text/field model (consistent with the other PDF tools).

## Tests

3 core unit tests over **PDFs assembled in-test with lopdf**: encrypt →
the output reports `is_encrypted()`, the correct password `decrypt`s it and a
wrong password fails; a distinct owner password also opens the document; and clear
errors on an empty password, garbage input, and already-encrypted input. Plus the
block drift-guard schema test. **CLI verified** end-to-end on the live IRS W-9
(`%PDF` output carrying an `/Encrypt` dictionary). `wafer build` instantiates the
chat block in the wafer runtime (1.11 MiB) — confirming the AES-256 path runs
under wasm32-wasip1.
