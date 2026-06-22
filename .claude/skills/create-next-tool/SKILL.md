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

**An "ffmpeg"-tagged image→GIF/animation tool often needs NO ffmpeg** (gif-from-images): the `image` crate
(features incl. `gif`) encodes animated GIFs purely — `use image::codecs::gif::{GifEncoder, Repeat};
enc.set_repeat(Repeat::Infinite); enc.encode_frame(Frame::from_parts(rgba, 0, 0,
Delay::from_numer_denom_ms(ms, 1)))`. Building it pure-Rust makes it run on ALL backends (incl. chat SW),
strictly better than ffmpeg-only. Multi-image-in + image-bytes-out → chat+CLI, **no page** (use
`Param::source_list("images", 1)` + `Vec<SourceFields>`, resolve each via `resolve_source`, like
image-collage/images-to-pdf). Don't reflexively skiplist a media tool as "ffmpeg" — check if `image`/a
pure crate covers it first.

**OpenPGP key generation (generate-pgp-key-pair):** rPGP `pgp = "0.14"` (default-features=false) + rand 0.8
+ chrono(`clock`) + smallvec. `SecretKeyParamsBuilder::default().key_type(KeyType::EdDSALegacy /
Rsa(bits)).can_certify(true).can_sign(true).primary_user_id(uid).subkey(SubkeyParamsBuilder…
.key_type(KeyType::ECDH(ECCCurve::Curve25519) / Rsa(bits)).can_encrypt(true).build()?)` →
`.build()?.generate(&mut OsRng)?` → `.sign(&mut rng, || passphrase)?` = `SignedSecretKey`. Armor:
`sk.to_armored_string(None.into())?`; public = `let pk: SignedPublicKey = sk.into(); pk.to_armored_string(
None.into())?`. Fingerprint: `sk.fingerprint().as_bytes()`. Tests need `use pgp::types::SecretKeyTrait` for
`.unlock()`. Curve25519 = fast (good for tests); RSA-4096 slow. Non-deterministic generator → **no page**.
Cross-verify a generated public key by feeding it to the existing `pgp-encrypt` tool.

**Dup-skiplist:** generate-totp (= existing totp-generator), generate-pgp-key… no — built. Always grep
`ls blocks/ | grep -i <topic>` before building; a near-dup of an existing block → skiplist + re-pick.

**PAGE BOOLEAN CHECKBOX DEFAULT:** the page generator renders a boolean param as `<input type=checkbox
checked[default]>` — i.e. the box is **checked iff the descriptor `Param::boolean(...).default(true)`**. So a
`default(true)` boolean shows CHECKED on load, and the web `run()` receives `"true"`; unchecked sends
`"false"`. In the web wrapper use **positive truthy** (`matches!(v, "true"|"1"|"on"|"yes")`). In Playwright,
a default-true checkbox is already checked — to test the off-path call `page.uncheck('#in-<name>')` (not just
leaving it). HTML-tokenizer tools (html-formatter pretty / html-minifier) are a clean pair: a forgiving
quote-aware tag scanner (`scan_tag` skipping quoted `>`), VOID elements don't indent, and pre/textarea/
script/style are emitted verbatim — HTML is NOT well-formed XML so quick-xml (used by xml-formatter) can't
parse it.
**DRIFT GOTCHA — number-param defaults serialize as `N.0`:** `Param::number("x").default(1.0)` renders
`"default": 1.0` in the schema. In the authored drift JSON write `"default": 1.0` (NOT `1`) — serde_json
treats `1` as an integer and `1.0` as a float, so they compare unequal and the drift test fails. (Integer
params with `.default(1)` correctly render `1`.) `color_quant = "1"` (NeuQuant) is wasm-safe — image color
quantization to N colors. HSL image edits: hand-roll RGB↔HSL (no extra dep) for hue-shift / sat / lightness.

## Orchestration / environment findings (2026-06-21)

**Hardware reality on this box: 2 CPUs, ~3.9 GB total RAM (~2.9 GB free), 78 GB disk.** A single Rust
release build (rustc + `wasm-opt` in wasm-pack + `cargo install cli`) peaks around 1–2 GB and saturates
both cores. **Parallel tool builds are NOT viable here** — two concurrent heavy builds risk OOM on ~3 GB
free, and the 2-core CPU means wall-clock barely improves. RAM-adaptive concurrency
(`free -m` → floor(available_GB / ~3)) computes ≈1 on this machine. So the loop is effectively
**sequential, one tool at a time**. (Also: `cargo install --path cli`, the page generator, and `git push`
all touch global/shared state — parallel builders would need separate worktrees AND serialized
CLI-install/generate/push, negating the gain.)

**The real win from sub-agents here is CONTEXT, not speed:** a thin dispatcher that spawns a FRESH
general-purpose sub-agent per tool (pick→build→test→commit→push, return a one-line result) keeps the
loop's own context tiny and effectively "clears context" every tool, letting the loop run indefinitely.
No races because only one builder runs at a time.

**5-hour usage limit:** there is NO local JSON recording the rolling 5-hour usage window / reset time.
`~/.claude/.credentials.json` holds only OAuth (`accessToken`/`refreshToken`/`expiresAt` = token expiry,
not the usage window) plus `subscriptionType=max`, `rateLimitTier=default_claude_max_20x`. The 5-hour
quota + reset are enforced server-side and only surfaced on a rate-limit error response — so the loop
can't pre-read remaining budget from disk; it must react to a limit error (back off / stop) when it hits one.

**More proven wasm-safe crates (this loop):** `toml = "0.8"` (config; TOML needs a table root + has no
null), `color_quant = "1"` (NeuQuant palette quantization), `kamadak-exif = "0.6"` (EXIF/TIFF read —
`Reader::new().read_from_container(Cursor)`, `field.tag`/`field.display_value().with_unit(&exif)`, GPS
rationals → decimal), `serde_json` `preserve_order` feature (keeps object key order). PDF *generation*
needs no new crate — build content streams with the already-proven `lopdf` (base-14 Helvetica, BT/Tf/Td/Tj/ET).
For exact RFC test vectors (e.g. RFC 7638 jwk-thumbprint) WebFetch the RFC rather than trusting memory —
a mis-remembered constant fails the vector.

## Sub-agent dispatch mode (preferred for long `/loop` runs)

To keep the loop's own context small (so it can run for hours without hitting the context window), the
loop should act as a **thin dispatcher**: per iteration, spawn ONE fresh general-purpose sub-agent that
builds the next tool end-to-end and returns only a one-line result. The heavy per-tool transcript
(scaffold, build logs, test output) stays inside the sub-agent and never enters the dispatcher's context.
**Sequential only** (one builder at a time) on this 2-CPU / ~4 GB box — parallel builders OOM and race on
the shared CLI/generator/git (see environment findings above).

Dispatcher per iteration:
1. `Agent(subagent_type:"general-purpose", description:"build next gizza tool", prompt: <BUILDER PROMPT below>)`.
2. Read the returned one-liner; if it says a tool was built+pushed or skiplisted, continue. If it says
   FAILED or hit a usage/rate limit, back off (longer ScheduleWakeup) and report.
3. `ScheduleWakeup` to re-enter `/loop` (dispatch the next one). Do NOT build inline anymore.

**The full long-running loop now lives in the sibling `create-tool-loop` skill** (dispatcher +
pacing + failure/limit back-off + task-leak cleanup + operational findings). Prefer invoking that
to run the loop. The BUILDER PROMPT below is kept here for reference and must stay in sync with it.

**FOREGROUND builds only (2026-06-22):** the builder MUST run every build in the foreground (Bash
timeout up to 600000 ms covers any single step) and must NOT use `run_in_background` or `sleep`/poll
loops — those leak as orphan "running" tasks that pile up by the hundreds. Kill any background job
before returning.

BUILDER PROMPT (self-contained — the sub-agent has a fresh context, so it must be told everything):
> Build the next gizza backlog tool end-to-end. Working dir /root/gizza-ai/gizza-ai; `source $HOME/.cargo/env`
> in every bash command; use absolute paths; for `wafer build` cd into blocks/<slug>/ first; cwd resets to
> /root after /tmp commands. Steps: (1) `python3 scripts/pick-next-tool.py 2>/dev/null | grep -v '^skip' | head -1`.
> (2) Read `.claude/skills/create-next-tool/SKILL.md` (esp. the Findings log) and follow the new-tool build
> procedure: classify type, `scripts/scaffold-tool.sh <slug> <type>`, implement core (+unit tests)/descriptor
> (+drift-guard schema test)/web/page, build (`cargo test`, then `wafer build` in blocks/<slug>/ and
> `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` — run heavy builds in background +
> poll), `cargo install --path cli`, `cargo run --manifest-path tools/generator/Cargo.toml -- .`, verify all
> applicable surfaces (CLI `gizza tool <slug>`; page Playwright `cd tests && xvfb-run npx playwright test
> tool-page-<slug>.spec.ts`). (3) Write `docs/checks/<today>-improve-<slug>-competitor-analysis.md`.
> (4) If it's a semantic dup of an existing block, instead append `<slug>  # reason` to
> `docs/tool-skiplist.txt`, commit that, and re-pick. (5) Honesty gate: if it can't be built+verified in
> ≤3 fix attempts, `git clean -fd blocks/<slug>`, skiplist or report, do NOT commit broken. (6) Clean
> per-block target dirs (`for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name block.wasm
> -exec rm -rf {} + ; done`), NEVER delete web/pkg. (7) `git add -A && git commit && git push origin
> feat/tool-creating`. Return ONE line only: `<slug>: built+pushed <short-sha>` OR `skiplisted <slug>: <reason>`
> OR `FAILED <slug>: <reason>`.
