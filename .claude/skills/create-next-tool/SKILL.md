---
name: create-next-tool
description: "Use when the user wants to build the next un-built gizza tool from the tools-to-build.csv backlog. Picks the next tool whose blocks/<slug>/ folder doesn't exist yet, builds it with the /new-tool procedure, fully enhances + verifies it with the /improve-tool procedure (competitor research + 3-surface checks), then commits and pushes on the current branch. No new branch, no PR. One tool per run."
---

# create-next-tool — build the next backlog tool end to end

Autonomous: build ONE tool per run from `tools-to-build.csv`, on the CURRENT branch, with NO
new branch and NO PR — just commits + a push. It orchestrates the two sibling skills: follow
`/new-tool`'s build steps, then the FULL `/improve-tool` procedure, but own the git yourself.
NEVER claim a step passed that you didn't run.

Read the two sibling skills for the actual recipes:
- `.claude/skills/new-tool/{SKILL.md,reference.md}` — the build procedure.
- `.claude/skills/improve-tool/{SKILL.md,reference.md}` — the verify+improve procedure.

Follow these steps in order:

0. **Toolchain.** This skill needs `cargo`, `wafer`, `wasm-pack`, `solobase`, `gizza`, Playwright,
   and `ffmpeg`. If any is missing, bootstrap once with `scripts/bootstrap-toolchain.sh` (details +
   gotchas in `docs/TOOLCHAIN-SETUP.md`); the very first run also needs a baseline `solobase build`
   so every existing block has its `target/block.wasm` + `web/pkg/` (else the generator hard-aborts
   and `gizza list` is incomplete).

1. **Pick the next tool.** From the gizza-ai repo root:
   ```bash
   scripts/pick-next-tool.py
   ```
   It prints `<slug>\t<name>\t<description>\t<type_hint>` for the next buildable tool, or a
   sentinel (`BACKLOG_COMPLETE` / `NO_BUILDABLE_REMAINING`) — report it and stop on a sentinel.
   The picker improves on a plain first-un-built scan (it logs every skip to stderr):
   - **built** = `blocks/<slug>/` committed in `git HEAD` (a half-built failure never counts, so a
     crashed run is retried, not skipped forever);
   - **curated skips** from `docs/tool-skiplist.txt` — confirmed duplicates of an existing tool
     (the exact-slug scan can't catch semantic near-dups like `pdf-to-text` ≈ `pdf-extract-text`);
   - **out-of-model** rows (need an ML model / pyodide — whisper, transformers-js, etc.) are
     **deferred** by default; gizza is pure-Rust + ffmpeg. Pass `--include-model` only if you
     intend to build one as a gpu chat-only block.
   `name` + `description` are your build inputs; `type_hint` (pure|ffmpeg|network|model) is a
   starting guess — still classify properly in step 2. `scripts/pick-next-tool.py --stats` shows
   the backlog breakdown.
   **If during the build you discover the tool is a semantic near-dup of an existing one, STOP, add
   a `<slug>  # duplicate of blocks/<other>` line to `docs/tool-skiplist.txt`, commit that, and
   re-run the picker** rather than shipping a redundant tool.

2. **Build** — follow `/new-tool` **steps 3–8** (classify type → `scripts/scaffold-tool.sh <slug> <type>`
   → implement `core`/`descriptor`/`web`/`page` → build → type-aware tests) using the `name` +
   `description` from step 1. **SKIP** `/new-tool` step 2 (branch) and steps 9–10 (push/PR/code-review)
   — git is owned by step 4 here.

   **Throughput note (verified):** the per-tool validation is `cd blocks/<slug> && cargo test
   --workspace` + `wafer build` (validates the chat block.wasm) + `wasm-pack build blocks/<slug>/web`
   + `cargo run --manifest-path tools/generator/Cargo.toml -- .` (renders the page) + `gizza tool`
   (CLI) + Playwright. These are minutes. The full **`solobase build` rebuilds the whole app wasm
   (`-Oz`+lto, ~25 min on 2 CPUs) and is the loop bottleneck** — a new block changes only its own
   block.wasm, which `wafer build` already validated, and CI runs `solobase build` on deploy. So for
   loop throughput, run `solobase build` **once at the start (baseline) and not on every tool**; if you
   skip it per-tool, say so in the report. The generator step still needs every block's `web/pkg/`
   (built once in the baseline; only the new tool's is added per run).

3. **Improve** — follow the FULL `/improve-tool` **Phases 1–5** on `<slug>`: verify the 3 surfaces
   (chat/LLM API, CLI, page query-params) + fix any breakage → research the top-5 competitors → diff +
   rank gaps (fit-to-model) → close every in-model capability/copy/UX/visual gap → regenerate the
   drift-guard → re-run the full test matrix. Write the competitor-analysis snapshot to
   `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`. **SKIP** `/improve-tool`'s "Gather
   + branch" step and **Phase 6** (PR). Its rules carry over: **NEVER copy competitor
   copy/branding/trademarks**; list out-of-model features, don't build them.

4. **Commit + push** on the CURRENT branch (no PR). Two commits keep history clear:
   ```bash
   git add blocks/<slug> tests/
   git commit -m "feat(<slug>): new tool"
   git add blocks/<slug> tests/ docs/checks/
   git commit -m "feat(<slug>): competitor improvements + analysis"
   git push
   ```

**Honesty + cleanup gate:** if the build (step 2) or verification (step 3 Phase 1) fails
unrecoverably (≤3 fix attempts per the sibling skills), **STOP, run `git clean -fd blocks/<slug>` to
remove the partial scaffold, and report the failure with the error.** NEVER commit a broken tool — a
committed broken tool's `blocks/<slug>/` would make the next run skip it forever. If a surface can't
be headlessly verified (gpu has no page; chat-ffmpeg can't run in a Service Worker — page + CLI only),
state it explicitly rather than claiming a pass. **One tool per run**; re-invoke (or `/loop`) for the
next.

**Page output formats:** the page driver renders `format = "image" | "video" | "audio"` (media,
with a download link) or anything else as `text`. Audio output (`format = "audio"` → `<audio
controls>`) was added with `extract-audio-from-video` — so **video→audio** ffmpeg tools are fully
supported now (set the page `format = "audio"`, give the block an `audio/*` mime via the core
`Format::mime()`, and use `build_media_envelope`; CLI-test with a video that actually has an audio
track — many small test clips are silent and will fail with "nothing to encode"). Still missing:
audio-**input** tools (audio-convert/normalize/…) need an `AssetKind::Audio` + `accept="audio/*"`
page input, which is unbuilt — keep those skiplisted.

**Known limitation (mitigated):** the picker matches built tools by exact slug, so a semantic
near-dup (e.g. `pdf-to-text` vs the built `pdf-extract-text`) isn't auto-detected. `docs/tool-skiplist.txt`
holds the confirmed dups found so far; when you spot a NEW one mid-build, add it there (step 1) rather
than shipping a redundant tool. Token-overlap auto-detection was tried and rejected — it false-flags
distinct tools (`age-calculator` is not a dup of `calculator`), so dups stay hand-curated.

## Findings log (2026-06, opus loop)

Reusable facts discovered while building ~20 tools. Trust these to skip dead ends.

**Page input field types (meta.toml `[[input]]`):** the generator renders each field by the
descriptor's Param type, NOT the meta — so Playwright must match: `Param::enumv` → `<select>`
(`page.selectOption('#in-<name>', value)`); `Param::boolean` → `<input type=checkbox>` (use
`page.check`/`uncheck`, default reflects `.default(true)`); `Param::integer`/`number` → a field
(`page.fill`); `Param::string` → `<input>` UNLESS the meta `[[input]]` sets `multiline = true`,
which renders a `<textarea>`. **Use `multiline = true` for any text/code/key field** so pasted
newlines are preserved (a plain `<input>` strips them — this is why multi-line keys can't go in a
plain field). Pages still need `format = "text"` or output is gated off.

**Crypto / encoding crates that instantiate in wafer (wasm32-wasip1):** `rsa` 0.9 (sign: pkcs1v15 +
pss, `features=["sha2"]`), `p256/p384/p521` (+`spki`+`pkcs8`, EC needs `features=["arithmetic",
"pkcs8"]`), `pgp` 0.14 (rPGP, `default-features=false`; encrypt/sign; for multi-recipient encryption
wrap primary/subkey in a small enum impl'ing `PublicKeyTrait` since the slice must be homogeneous),
`lopdf` 0.36 encryption (`Document::encrypt` + `EncryptionVersion::V5` AES-256, not feature-gated),
`hmac`+`sha1`+`sha2`+`base32` (TOTP), `pem`, `quick-xml` 0.36, `qrcode` 0.14 (`features=["svg"]` → SVG
string, no image dep), `zip` 8 (`default-features=false, features=["deflate"]`, read + write).

**QR decode: use `quircs`, NOT `rqrr`.** `rqrr` compiles to wasm but pulls filesystem WASI imports
(`path_open`/`fd_close`) the wafer runtime doesn't provide → fails to instantiate. `quircs` decodes
from a raw grayscale buffer (`image::load_from_memory(..).to_luma8()` → `Quirc::identify`) and
instantiates fine. (General rule: an engine crate must INSTANTIATE, not just compile — `wafer build`
is the gate; watch for `cannot find import wasi_snapshot_preview1::{poll_oneoff,path_open,fd_close}`.)

**Time:** `std::time::SystemTime::now()` and `chrono::Utc::now()` DO instantiate in the chat block
(wafer provides the clock import) and work natively in the CLI. But the PAGE target
(wasm32-unknown-unknown) has NO std clock — get time in the web crate via `js_sys::Date::now()/1000`
(add `js-sys` to web/Cargo.toml). Make the core take an explicit timestamp so it's deterministic and
each surface supplies its own clock.

**CLI verification fetch is SSRF-guarded:** `gizza tool … url=…` only fetches PUBLIC http(s); `data:`
URLs and localhost are rejected ("request to private/internal address is not allowed"), and GitHub
`/archive/` URLs redirect (the fetcher doesn't follow) — use a direct host. Handy public test inputs:
zip → `https://codeload.github.com/octocat/Hello-World/zip/refs/heads/master`; live QR PNG →
`https://api.qrserver.com/v1/create-qr-code/?data=...&size=300x300`.

**Tool shapes that recur:** SVG/PDF/image-bytes output → `build_media_envelope` (mime
`image/svg+xml` / `application/pdf` / `image/png`), chat+CLI, NO page (image-bytes have no page render
mode); hand-build SVG with `format!`/`r##"..."##` (colours like `#fff` need the `##` raw delimiter).
File-input→JSON (strings/unzip/detect-file-type) use `AssetKind::Any` (there is no `AssetKind::File`)
+ flat `Resp` via `GuestResult::respond`. Text+passphrase/key tools can reuse another block's core via
`path = "../../<other>/core"` (text-encrypt reuses encrypt-file's AES-GCM core).

**Disk:** per-block `target/` dirs are ~0.3–2.5 GB each and fill the disk after ~12 tools. Reclaim
with: `for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name block.wasm -exec rm -rf
{} +; done` (keeps the committed `block.wasm` the CLI build embeds).

**Ops gotcha:** `wafer build` must run from inside `blocks/<slug>/`; `cargo install --path cli` and
`wasm-pack build blocks/<slug>/web …` must run from the repo ROOT — after any `/tmp` poll the shell
cwd resets to `/root`, so always cd to the absolute repo path before those.

**Multi-input ffmpeg (e.g. video-concat) is effectively un-buildable here:** the page file-input is a
single upload and ffmpeg can't run in the chat SW, so it'd be CLI-only — skiplist + defer.

**NEVER delete `blocks/<slug>/web/pkg`** during cleanup — the page generator copies each tool's
`web/pkg/*` into `pkg/tools/<slug>/`, so deleting one tool's pkg makes the *next* generator run fail
(`No such file or directory` copying that slug). The disk-cleanup loop above only touches `target/`,
which is correct; do not additionally `rm -rf` any `web/pkg`. If a pkg does go missing, rebuild it with
`wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` before re-running the generator.

**ECDSA signing (ecdsa-sign):** use `p256`/`p384 = { default-features=false, features=["arithmetic",
"pkcs8","ecdsa","pem"] }` (the `pem` feature is what enables `SigningKey::from_pkcs8_pem`; there is no
`sec1` feature flag) + `ecdsa = { features=["der"] }`. Sign with `use pXXX::ecdsa::{signature::Signer,
Signature, SigningKey}; use pXXX::pkcs8::DecodePrivateKey;` then `let sig: Signature = sk.sign(msg)`
(deterministic RFC-6979 — no RNG, instantiates clean in wafer, no getrandom needed). DER bytes =
`sig.to_der().as_bytes().to_vec()` (NOT `.to_bytes()`); raw r||s = `sig.to_bytes().to_vec()` (64 B P-256,
96 B P-384). **Skip P-521**: `p521`'s ECDSA `Signer` impl is randomized-only (gated on `getrandom`, uses
`OsRng`, no RFC-6979 path), so it breaks determinism and pulls getrandom — support P-256/P-384 only.
Verify DER output cross-tool with `openssl dgst -sha256 -verify pub.pem -signature sig.der msg`.

**Ed25519 (ed25519-key-pair-generator):** `ed25519-dalek = { default-features=false, features=["pkcs8",
"pem","rand_core"] }` + `rand="0.8"`. `SigningKey::generate(&mut rand::rngs::OsRng)`, then
`sk.to_pkcs8_pem(LineEnding::LF)` (import `ed25519_dalek::pkcs8::spki::der::pem::LineEnding` +
`EncodePrivateKey`/`EncodePublicKey`), `vk.to_public_key_pem(...)`, raw via `sk.to_bytes()`/`vk.to_bytes()`
(32 B each). Re-parse with `SigningKey::from_pkcs8_pem` (needs `DecodePrivateKey`). Like key-gen generally
(see generate-rsa-key-pair): **no page** — a zero-input non-deterministic generator doesn't fit the page's
recompute-on-input model. No-input chat tool: empty `#[derive(Deserialize,Default)] #[serde(default)] struct
Args {}`, descriptor `ToolDescriptor::new(Input::None)` (no params) → authored schema is just
`{"type":"object","properties":{},"additionalProperties":false}`.

**More proven wasm-safe crates (this loop):** `mail-parser = "0.11"` (default-features=false) — RFC 5322/MIME
email parsing; `MessageParser::default().parse(bytes)`, `msg.from()/to()/cc()` return `Option<&Address>`
(use `.iter()` → `Addr::name()/address()`), `msg.body_text(0)/body_html(0)`, `msg.attachments()` →
`MessagePart` (`use mail_parser::MimeHeaders` for `.attachment_name()/.content_type()`; ContentType has
`.ctype()/.subtype()`), `msg.date().to_rfc3339()`. `htmd = "0.5"` (HTML→Markdown) + `nanohtml2text = "0.1"`
(`html2text(&s)`, HTML→plain) + `quick-xml = "0.40"` (default-features=false) all instantiate in wafer.

**EPUB / ZIP-container parsing (epub-to-markdown):** an EPUB is a ZIP — read with
`zip::ZipArchive::new(Cursor::new(bytes))`, find OPF via `META-INF/container.xml` (`<rootfile full-path>`),
parse the OPF with quick-xml for `<manifest><item id href media-type>` + `<spine><itemref idref>` to get
**reading order** (don't use ZIP/alphabetical order), resolve hrefs relative to the OPF dir, convert each
XHTML. quick-xml: match elements by local name (strip `ns:` prefix), handle both `Event::Start` and
`Event::Empty` for self-closing `<item/>`. Binary-file-in/text-out → **no page** (file-input pattern).