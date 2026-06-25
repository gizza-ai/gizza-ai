#!/usr/bin/env bash
# gen-seo.sh — gizza list-driven SEO file generator
# Writes pkg/sitemap.xml, pkg/robots.txt, pkg/llms.txt
# Run from repo root (or pass repo root as first argument).
set -euo pipefail

BASE_URL="${BASE_URL:-https://gizza.ai}"
GIZZA="${GIZZA:-gizza}"

# Allow passing an explicit repo root for testing; default to script's parent dir
REPO_ROOT="${1:-$(cd "$(dirname "$0")/.." && pwd)}"

PKG_DIR="$REPO_ROOT/pkg"
mkdir -p "$PKG_DIR"

# Single gizza list call — boot wasmi runtime exactly once
tools_json="$("$GIZZA" list --json-out)"

# Derive sorted slug list from the stored JSON (no per-tool gizza invocation)
mapfile -t all_slugs < <(printf '%s' "$tools_json" | jq -r '.[].name | sub("^gizza-ai/"; "")' | sort)

# Collect slugs that have a standalone page (blocks/<slug>/page/meta.toml exists)
page_slugs=()
for slug in "${all_slugs[@]}"; do
  [[ -z "$slug" ]] && continue
  if [[ -f "$REPO_ROOT/blocks/$slug/page/meta.toml" ]]; then
    page_slugs+=("$slug")
  fi
done

# --- sitemap.xml ---
{
  printf '<?xml version="1.0" encoding="UTF-8"?>\n'
  printf '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
  printf '  <url><loc>%s/</loc></url>\n' "$BASE_URL"
  printf '  <url><loc>%s/chat</loc></url>\n' "$BASE_URL"
  printf '  <url><loc>%s/tools/</loc></url>\n' "$BASE_URL"
  for slug in "${page_slugs[@]}"; do
    printf '  <url><loc>%s/tools/%s/</loc></url>\n' "$BASE_URL" "$slug"
  done
  printf '</urlset>\n'
} > "$PKG_DIR/sitemap.xml"

# --- robots.txt ---
{
  printf 'User-agent: *\n'
  printf 'Allow: /\n'
  printf 'Sitemap: %s/sitemap.xml\n' "$BASE_URL"
} > "$PKG_DIR/robots.txt"

# --- llms.txt (llmstxt.org format) ---
{
  printf '# gizza.ai\n\n'
  printf '> gizza.ai is a browser-local AI chat assistant with a growing library of\n'
  printf '> privacy-first tools. Everything runs in your browser or via the gizza CLI —\n'
  printf '> no data leaves your device.\n\n'

  printf '## Tools\n\n'

  # Emit one entry per tool from gizza list (order: sorted slugs)
  # Page tools: link to index.md; chat-only tools: description, no link
  for slug in "${all_slugs[@]}"; do
    [[ -z "$slug" ]] && continue
    # Look up description from the already-loaded JSON — no additional gizza invocation
    desc="$(printf '%s' "$tools_json" | jq -r --arg n "gizza-ai/$slug" '.[] | select(.name == $n) | .description')"
    if [[ -f "$REPO_ROOT/blocks/$slug/page/meta.toml" ]]; then
      printf -- '- [%s](%s/tools/%s/index.md): %s\n' "$slug" "$BASE_URL" "$slug" "$desc"
    else
      printf -- '- %s: %s (available in chat + via the gizza CLI)\n' "$slug" "$desc"
    fi
  done

  printf '\n## Resources\n\n'
  printf -- '- [GitHub](https://github.com/gizza-ai/gizza-ai): source code and issue tracker\n'
  printf -- '- [Discord](https://discord.com/invite/jKqMcbrVzm): community and support\n'
  printf -- '- [Donate](https://github.com/sponsors/Jsuppers): support the project\n'
  printf -- '- [CLI README](https://github.com/gizza-ai/gizza-ai/blob/main/cli/README.md): gizza CLI reference\n'
  printf -- '- [SKILL.md](https://github.com/gizza-ai/gizza-ai/blob/main/SKILL.md): agent integration guide\n\n'
  printf 'For the authoritative machine-readable tool catalog, see `/tools/_index.json` or\n'
  printf 'run `gizza list --json-out`. The catalog is build-time static and always up to date.\n'
} > "$PKG_DIR/llms.txt"

echo "gen-seo.sh: wrote $PKG_DIR/sitemap.xml, $PKG_DIR/robots.txt, $PKG_DIR/llms.txt"
