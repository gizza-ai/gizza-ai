#!/usr/bin/env bash
# gen-seo.sh — gizza list-driven SEO file generator
# Writes pkg/sitemap.xml, pkg/robots.txt, pkg/llms.txt, pkg/llms-full.txt
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

# Read a quoted scalar field from a TOML meta file (best-effort, first match).
meta_field() {
  # $1 = meta.toml path, $2 = field name
  [[ -f "$1" ]] || return 0
  sed -n -E "s/^[[:space:]]*$2[[:space:]]*=[[:space:]]*\"(.*)\"[[:space:]]*\$/\1/p" "$1" | head -n1
}

# Collect slugs that have a standalone page directly from the filesystem
# (blocks/<slug>/page/meta.toml). Deriving these from disk — not from the gizza
# catalog — guarantees every page tool lands in the sitemap even if the CLI
# catalog lags behind the on-disk blocks/*/page tools.
page_slugs=()
while IFS= read -r meta_file; do
  [[ -z "$meta_file" ]] && continue
  slug="${meta_file#"$REPO_ROOT/blocks/"}"
  slug="${slug%/page/meta.toml}"
  page_slugs+=("$slug")
done < <(
  [[ -d "$REPO_ROOT/blocks" ]] &&
    find "$REPO_ROOT/blocks" -mindepth 3 -maxdepth 3 -path '*/page/meta.toml' -type f | sort
)

# Membership lookup for page slugs (used when emitting llms.txt)
is_page_slug() {
  local s
  for s in "${page_slugs[@]}"; do
    [[ "$s" == "$1" ]] && return 0
  done
  return 1
}

# Last commit date (ISO 8601) touching a path — used for sitemap <lastmod>.
# Empty when history is unavailable (shallow clone, no git); the tag is then
# omitted rather than emitting a wrong date.
git_lastmod() {
  git -C "$REPO_ROOT" log -1 --format=%cI -- "$1" 2>/dev/null || true
}

emit_url() {
  # $1 = loc, $2 = lastmod (may be empty)
  if [[ -n "$2" ]]; then
    printf '  <url><loc>%s</loc><lastmod>%s</lastmod></url>\n' "$1" "$2"
  else
    printf '  <url><loc>%s</loc></url>\n' "$1"
  fi
}

# --- sitemap.xml ---
site_mod="$(git_lastmod site)"
{
  printf '<?xml version="1.0" encoding="UTF-8"?>\n'
  printf '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
  blocks_mod="$(git_lastmod blocks)"
  emit_url "$BASE_URL/" "$site_mod"
  emit_url "$BASE_URL/chat" "$site_mod"
  emit_url "$BASE_URL/tools/" "$blocks_mod"
  # Category hub pages (/tools/<category>/) from the generator's _hubs.json.
  # Skipped gracefully when the file is missing (generator not yet run); hubs
  # aggregate many tools, so they share the blocks lastmod like /tools/.
  hubs_json="$PKG_DIR/tools/_hubs.json"
  if [[ -f "$hubs_json" ]]; then
    while IFS= read -r hub_slug; do
      [[ -z "$hub_slug" ]] && continue
      emit_url "$BASE_URL/tools/$hub_slug/" "$blocks_mod"
    done < <(jq -r '.[].slug' "$hubs_json")
  fi
  for slug in "${page_slugs[@]}"; do
    emit_url "$BASE_URL/tools/$slug/" "$(git_lastmod "blocks/$slug/page")"
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

  # Emit one entry per tool, over the union of gizza-listed slugs and on-disk
  # page slugs (sorted, deduped). Page tools link to index.md — even when the
  # gizza catalog omits them — falling back to page/meta.toml description/title
  # for the summary. Chat-only tools (in gizza list, no page) keep their
  # description-only line as before.
  mapfile -t llms_slugs < <(printf '%s\n' "${all_slugs[@]}" "${page_slugs[@]}" | grep -v '^$' | sort -u)
  for slug in "${llms_slugs[@]}"; do
    # Look up description from the already-loaded JSON — no additional gizza invocation
    desc="$(printf '%s' "$tools_json" | jq -r --arg n "gizza-ai/$slug" '.[] | select(.name == $n) | .description')"
    if is_page_slug "$slug"; then
      # Fall back to meta.toml when gizza has no entry (or a null description)
      if [[ -z "$desc" || "$desc" == "null" ]]; then
        meta="$REPO_ROOT/blocks/$slug/page/meta.toml"
        desc="$(meta_field "$meta" description)"
        [[ -z "$desc" ]] && desc="$(meta_field "$meta" title)"
      fi
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
  printf -- '- [SKILL.md](https://github.com/gizza-ai/gizza-ai/blob/main/SKILL.md): agent integration guide\n'
  printf -- '- [llms-full.txt](%s/llms-full.txt): every tool page'\''s full markdown documentation in one file\n\n' "$BASE_URL"
  printf 'For the authoritative machine-readable tool catalog, see `/tools/_index.json` or\n'
  printf 'run `gizza list --json-out`. The catalog is build-time static and always up to date.\n'
  printf "Every tool also serves a JSON Schema descriptor at \`/tools/<slug>/tool.json\`\n"
  printf '(identity, URLs, CLI invocation and tool parameters).\n'
} > "$PKG_DIR/llms.txt"

# --- llms-full.txt (header + every pkg/tools/*/index.md, slug order) ---
# The generator writes the per-tool index.md files before this script runs in
# deploy; when they are absent (e.g. gen-seo.sh run standalone), skip with a
# warning rather than emitting a header-only file.
mapfile -t index_mds < <(
  [[ -d "$PKG_DIR/tools" ]] &&
    find "$PKG_DIR/tools" -mindepth 2 -maxdepth 2 -name index.md -type f | sort
)
if ((${#index_mds[@]} == 0)); then
  echo "gen-seo.sh: warning: no pkg/tools/*/index.md files — skipping llms-full.txt (run the tool-page generator first)" >&2
else
  {
    printf '# gizza.ai — full tool documentation'
    for md in "${index_mds[@]}"; do
      # $(<file) drops trailing newlines, so documents are separated by
      # exactly one '---' block regardless of each file's final newline.
      printf '\n\n---\n\n%s' "$(<"$md")"
    done
    printf '\n'
  } > "$PKG_DIR/llms-full.txt"
fi

echo "gen-seo.sh: wrote $PKG_DIR/sitemap.xml, $PKG_DIR/robots.txt, $PKG_DIR/llms.txt${index_mds[0]:+, $PKG_DIR/llms-full.txt}"
