# Ops / environment findings (disk, cwd, verification inputs, limits)

**Disk:** per-block `target/` dirs are ~0.3–2.5 GB each and fill a small disk after ~12 tools. Reclaim
with: `for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name block.wasm -exec rm -rf
{} +; done` (keeps the committed `block.wasm` the CLI build embeds).

**NEVER delete `blocks/<slug>/web/pkg`** during cleanup — the page generator copies each tool's
`web/pkg/*` into `pkg/tools/<slug>/`, so deleting one tool's pkg makes the *next* generator run fail
(`No such file or directory` copying that slug). The disk-cleanup loop above only touches `target/`,
which is correct; do not additionally `rm -rf` any `web/pkg`. If a pkg does go missing, rebuild it with
`wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` before re-running the generator.

**cwd gotcha:** `cargo build --target wasm32-wasip1 --release` (the block-wasm build; optional
equivalent shorthand `wafer build` if you have the `wafer` CLI installed from a sibling `wafer-run`
checkout — not required in this repo) must run from inside `blocks/<slug>/`; `cargo install --path
cli` and `wasm-pack build blocks/<slug>/web …` must run from the repo ROOT — after any `/tmp` command
the shell cwd resets, so always cd to the absolute repo path before those.

**CLI verification fetch is SSRF-guarded:** `gizza tool … url=…` only fetches PUBLIC http(s); `data:`
URLs and localhost are rejected ("request to private/internal address is not allowed"), and GitHub
`/archive/` URLs redirect (the fetcher doesn't follow) — use a direct host. Handy public test inputs:
zip → `https://codeload.github.com/octocat/Hello-World/zip/refs/heads/master`; live QR PNG →
`https://api.qrserver.com/v1/create-qr-code/?data=...&size=300x300`. Also (2026-07-20):
`raw.githubusercontent.com` serves proper `image/png` (public repos' test images — e.g.
`sbrunner/deskew/master/tests/deskew-N.png`, real skewed scans WITH published expected angles),
and `https://dummyimage.com/900x300/ffffff/000000.jpg&text=hello` serves `image/jpg` (JPEG
secondary-format + rendered-text cases; all-white `/ffffff/ffffff.png` = blank-input case).

**Hardware / concurrency (probe, don't assume):** a single Rust release build (rustc + `wasm-opt` in
wasm-pack + `cargo install cli`) peaks around 1–2 GB and saturates its cores. On a small box (the
original loop ran on 2 CPUs / ~3.9 GB RAM) parallel tool builds OOM; on any box, parallel builders
race on the shared `cargo install`/page-generator/`git push` state — so the loop is **sequential, one
tool at a time** regardless of hardware. The real win from sub-agents is CONTEXT, not speed: a fresh
builder per tool keeps the dispatcher's context tiny so the loop runs indefinitely.

**5-hour usage limit:** there is NO local JSON recording the rolling 5-hour usage window / reset time.
`~/.claude/.credentials.json` holds only OAuth tokens (token expiry, not the usage window). The quota
+ reset are enforced server-side and only surfaced on a rate-limit error response — the loop can't
pre-read remaining budget from disk; it must react to a limit error (back off / stop) when it hits one.

**Fresh checkout / worktree gotchas (2026-07-02, from the first isolated builder run):**
- `tests/package-lock.json` is deliberately gitignored — use `npm install` in `tests/` (what CI
  does), NOT `npm ci` (which hard-requires a lockfile and fails).
- `cargo install --path cli` embeds only the blocks whose `target/block.wasm` exists. A fresh
  checkout has just the ~33 COMMITTED wasm fixtures, so the install produces (and globally
  overwrites `~/.cargo/bin/gizza` with) a CLI missing most tools. Either run the baseline per-block
  build loop first (`for dir in blocks/*/; do (cd "$dir" && cargo build --target wasm32-wasip1
  --release && mkdir -p target && cp target/wasm32-wasip1/release/*.wasm target/block.wasm); done` —
  see `docs/TOOLCHAIN-SETUP.md` step 7), or reinstall from the full checkout when done.
- The page generator prints a "web/pkg not found (skipping WASM copy)" warning per block whose
  wasm-pack output is missing — in a fresh checkout that's ~300 warnings. Harmless for the tool
  you're building (its own page still renders); noisy but expected without the baseline build.

**wafer-run git rev drift (2026-07-18):** a new block lockfile resolved
`wafer-block`/`wafer-block-macro`/`wafer-sdk` to wafer-run rev `10eb4e3d`, which compiled but made
`wafer build` validation fail with `skill parameters JSON parse error` / `expected value at line 1
column 1` from `__wafer_info()`. Existing validated blocks used rev `a5fa3ae30bd9f05033dc9ec2d5934bc47b60c189`.
Fix new blocks with:
`cargo update -p wafer-sdk --precise a5fa3ae30bd9f05033dc9ec2d5934bc47b60c189 && cargo update -p wafer-block --precise a5fa3ae30bd9f05033dc9ec2d5934bc47b60c189 && cargo update -p wafer-block-macro --precise a5fa3ae30bd9f05033dc9ec2d5934bc47b60c189`,
then rerun `cargo test --workspace` and `wafer build`.

**wafer CLI/local rev mismatch (2026-07-18):** on the continuation box, new pure/ffmpeg blocks
resolved wafer-run rev `915d9925`, which compiled but made the installed `wafer` CLI fail validation
with `Failed to deserialize BlockInfo from __wafer_info() output: expected value at line 1 column 1`.
Pinning the new block lockfile to the rev used by already-valid neighboring blocks fixed validation:
`cargo update -p wafer-sdk --precise 48926f4f && cargo update -p wafer-block --precise 48926f4f && cargo update -p wafer-block-macro --precise 48926f4f`. After pinning, rerun tests and `wafer build`.
If an ffmpeg skill also fails under that older macro with ``capabilities(...)` and `skill(...)` are
mutually exclusive`, the local `wafer` CLI is older than some existing source patterns; prefer
skiplisting a page-unverifiable HEVC/libx265-style tool rather than shipping an unvalidated block.

**No-page URL/ref tools must pass BOTH wafer validation and runtime CLI (2026-07-18).** For a new
PDF Document-source tool, rev `915d9925` compiled the required `capabilities(network, ...)` form but
`wafer build` validation failed with empty `__wafer_info()` (`expected value at line 1 column 1`).
Removing `capabilities(...)` let the wasm validate at rev `48926f4f`, but `gizza tool ... url=...`
then failed at runtime with `stream_init failed for wafer-run/network` (the same failure existing
no-page PDF URL/ref blocks show on this box). Treat this as not verifiable for new no-page
Document/File URL/ref tools until the wafer validation/runtime revs align; clean the scaffold and
skiplist rather than committing a block that only passes one side of the gate.

**Wafer rev drift RESOLVED at the root (2026-07-19).** The three 2026-07-18 notes above are
superseded: the correct pin target is always `wafer-run-pin.txt` (repo root), never "the rev
used by neighboring blocks". `scripts/scaffold-tool.sh` now pins a fresh block's lockfile to
that pin automatically, and PR CI re-pins every changed block before building + fails if any
committed `blocks/*/Cargo.lock` disagrees with the pin. If `wafer build` validation fails at
the pin rev itself, that's a wafer-run bug to fix there (bump the pin via the procedure in
the workspace memory), not something to route around with ad-hoc `--precise` revs. Block
Cargo.locks stay gitignored/uncommitted; don't force-add them.

**Shared CARGO_TARGET_DIR makes `cp target/wasm32-wasip1/release/*.wasm` a trap (2026-07-20):**
on boxes where cargo uses a shared target dir (e.g. `~/.cache/gizza-cargo-target`, the disk-space
fix), `blocks/<slug>/target/wasm32-wasip1/` doesn't exist and a `*.wasm` glob against the SHARED
release dir silently copies the alphabetically-first OTHER block's wasm into `target/block.wasm`
(caught only by size mismatch). Copy by exact name — `gizza_ai_<slug_underscored>_block.wasm` —
and `cmp` it against the source after copying. CI is unaffected (per-block target dirs).

**Stale local `wafer` CLI false-fails validation (2026-07-20):** `wafer build` failing with
`Failed to deserialize BlockInfo from __wafer_info()` on a NEW block whose Cargo.lock is at the
`wafer-run-pin.txt` rev is not necessarily a block problem — verify by running `wafer build` on a
known-good COMMITTED block (e.g. yesterday's shipped tool): if that fails identically, the local
wafer CLI is stale and its verdict is not load-bearing (CI never runs it; `cargo test` + the
wasm32-wasip1 build + `gizza tool` runtime execution are the real gates). Only if the pin-rev
block fails while known-good blocks pass is it a real wafer-run bug (fix there, per the 07-19 note).

**Public AUDIO test URLs (2026-07-20):** kozco.com serves proper audio/* content-types and works with
the SSRF-guarded fetch — `https://www.kozco.com/tech/piano2.wav` (6.3 s, 48 kHz stereo),
`https://www.kozco.com/tech/organfinale.wav` (13 s, 44.1 kHz) and
`https://www.kozco.com/tech/piano2-CoolEdit.mp3`; www2.cs.uic.edu is flaky/unreachable. Wikimedia
(`upload.wikimedia.org`) 403s the fetcher's UA. filesamples.com works for m4a/ogg (audio/x-m4a,
audio/ogg) but serves .flac as `application/octet-stream`, which the AssetKind::Audio MIME-class
check rightly rejects — for flac/ogg/m4a SUCCESS coverage commit tiny generated fixtures (3.5 s
lavfi sine via local ffmpeg, ~10-220 KB) under `core/tests/fixtures/` + an integration test
(precedent: encrypted-zip's committed fixture).
