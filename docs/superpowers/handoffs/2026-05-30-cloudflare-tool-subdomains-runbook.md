# Cloudflare tool-subdomains — operator runbook

Manual, account-level steps (done by a human; the CI changes are already in the repo).

## One-time setup

1. **Create the Pages project** named `gizza-ai` (Cloudflare dashboard → Workers & Pages
   → Create → Pages → Direct Upload, or let the first `wrangler pages deploy` create it).
2. **Custom domains** (Pages project → Custom domains):
   - Add `gizza.ai`
   - Add `www.gizza.ai`
   - Add `*.gizza.ai` (wildcard) — this is what makes `calculator.gizza.ai` etc. resolve.
3. **DNS** (gizza.ai zone): ensure proxied CNAMEs exist for `@`, `www`, and `*` pointing at
   the Pages project (`gizza-ai.pages.dev`). Cloudflare usually adds these when you attach
   custom domains.
4. **Repo secrets** (GitHub → repo → Settings → Secrets → Actions):
   - `CLOUDFLARE_API_TOKEN` — token with "Cloudflare Pages: Edit" permission.
   - `CLOUDFLARE_ACCOUNT_ID` — your account id.
5. **Retire GitHub Pages**: repo Settings → Pages → set Source to "None" (the old
   `Deploy to GitHub Pages` workflow has been replaced by `Deploy to Cloudflare Pages`).

## How the subdomain routing works

- The deploy uploads `pkg/` as static assets and the sibling `functions/` directory as
  Pages Functions. `functions/_middleware.js` runs on every request, reads the `Host`
  header via `functions/routing.mjs`, and:
  - `gizza.ai` / `www.gizza.ai` / `*.pages.dev` / `localhost` → serve the app unchanged.
  - `<tool>.gizza.ai` → internally serve `/tools/<tool>/...` (e.g. `calculator.gizza.ai/`
    → `/tools/calculator/index.html`).
  - any other host → 302 redirect to `https://gizza.ai/`.
- The CI runs wrangler with `workingDirectory: gizza-ai` so the `functions/` dir is found
  next to `pkg/`. If you ever deploy by hand, run `wrangler pages deploy pkg
  --project-name=gizza-ai` from the `gizza-ai/` repo root (NOT from a parent dir), or the
  Functions won't be picked up.

## Verify after first deploy

- `https://gizza.ai/` → chat app loads, "Tools" section links to subdomains.
- `https://calculator.gizza.ai/` → calculator page; typing `2+2*3` shows `8`.
- `https://clock.gizza.ai/` → live UTC timestamp (RFC-3339, e.g. `2026-05-30T12:00:00+00:00`).
- `https://gizza.ai/sitemap.xml` lists apex + every tool subdomain.
- `https://gizza.ai/robots.txt` references the sitemap.
- View source on a tool page: `<title>`, meta description, and JSON-LD (`WebApplication`)
  present in the static HTML (no JS needed).

## Adding a future tool

Create `blocks/<tool>/{core,web,page}/` following calculator's shape:
- `core/` — a pure rlib crate with the logic (member of the block's cargo workspace, no
  own `[workspace]` table).
- `web/` — a `wasm-bindgen` cdylib (`crate-type=["cdylib"]`) depending on `../core`.
- `page/meta.toml` + `page/content.md`.
Also make the block's `[workspace]` include `members = [".", "core", "web"]`.

The build (`just build-tools`), sitemap, index "Tools" interlink, and subdomain routing all
pick it up automatically — no per-tool wiring. After deploy, add `<tool>.gizza.ai` is
already covered by the `*.gizza.ai` wildcard custom domain, so no DNS change is needed.
