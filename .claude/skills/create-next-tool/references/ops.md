# Ops / environment findings (disk, cwd, verification inputs, limits)

**Disk:** per-block `target/` dirs are ~0.3–2.5 GB each and fill a small disk after ~12 tools. Reclaim
with: `for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name block.wasm -exec rm -rf
{} +; done` (keeps the committed `block.wasm` the CLI build embeds).

**NEVER delete `blocks/<slug>/web/pkg`** during cleanup — the page generator copies each tool's
`web/pkg/*` into `pkg/tools/<slug>/`, so deleting one tool's pkg makes the *next* generator run fail
(`No such file or directory` copying that slug). The disk-cleanup loop above only touches `target/`,
which is correct; do not additionally `rm -rf` any `web/pkg`. If a pkg does go missing, rebuild it with
`wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` before re-running the generator.

**cwd gotcha:** `wafer build` must run from inside `blocks/<slug>/`; `cargo install --path cli` and
`wasm-pack build blocks/<slug>/web …` must run from the repo ROOT — after any `/tmp` command the shell
cwd resets, so always cd to the absolute repo path before those.

**CLI verification fetch is SSRF-guarded:** `gizza tool … url=…` only fetches PUBLIC http(s); `data:`
URLs and localhost are rejected ("request to private/internal address is not allowed"), and GitHub
`/archive/` URLs redirect (the fetcher doesn't follow) — use a direct host. Handy public test inputs:
zip → `https://codeload.github.com/octocat/Hello-World/zip/refs/heads/master`; live QR PNG →
`https://api.qrserver.com/v1/create-qr-code/?data=...&size=300x300`.

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
  overwrites `~/.cargo/bin/gizza` with) a CLI missing most tools. Either run the baseline
  `solobase build` first, or reinstall from the full checkout when done.
- The page generator prints a "web/pkg not found (skipping WASM copy)" warning per block whose
  wasm-pack output is missing — in a fresh checkout that's ~300 warnings. Harmless for the tool
  you're building (its own page still renders); noisy but expected without the baseline build.
