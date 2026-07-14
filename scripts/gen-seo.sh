#!/usr/bin/env bash
# gen-seo.sh — gizza list-driven SEO file generator
# Writes pkg/sitemap.xml, pkg/robots.txt, pkg/feed.xml, pkg/llms.txt,
# pkg/llms-full.txt and the IndexNow key file.
# Run from repo root (or pass repo root as first argument).
set -euo pipefail

BASE_URL="${BASE_URL:-https://gizza.ai}"
GIZZA="${GIZZA:-gizza}"

# Allow passing an explicit repo root for testing; default to script's parent dir
REPO_ROOT="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
BLOCKS_DIR="${BLOCKS_DIR:-$REPO_ROOT/blocks}"

# Git repo containing the blocks tree (post-split this is the pinned gizza-ai
# checkout, a different repo from $REPO_ROOT) + the blocks dir's path inside it.
BLOCKS_GIT_ROOT="$(git -C "$BLOCKS_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
if [ -n "$BLOCKS_GIT_ROOT" ]; then
  BLOCKS_GIT_PREFIX="$(realpath --relative-to="$BLOCKS_GIT_ROOT" "$BLOCKS_DIR")"
else
  BLOCKS_GIT_ROOT="$REPO_ROOT"; BLOCKS_GIT_PREFIX="blocks"   # non-git fixture dirs (tests)
fi

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
  slug="${meta_file#"$BLOCKS_DIR/"}"
  slug="${slug%/page/meta.toml}"
  page_slugs+=("$slug")
done < <(
  [[ -d "$BLOCKS_DIR" ]] &&
    find "$BLOCKS_DIR" -mindepth 3 -maxdepth 3 -path '*/page/meta.toml' -type f | sort
)

# Membership lookup for page slugs (used when emitting llms.txt)
is_page_slug() {
  local s
  for s in "${page_slugs[@]}"; do
    [[ "$s" == "$1" ]] && return 0
  done
  return 1
}

# Last commit date (ISO 8601) touching a path in $REPO_ROOT — used for
# sitemap <lastmod> on site (non-blocks) paths. Empty when history is
# unavailable (shallow clone, no git); the tag is then omitted rather than
# emitting a wrong date.
git_lastmod() {
  git -C "$REPO_ROOT" log -1 --format=%cI -- "$1" 2>/dev/null || true
}

# First commit touching a path in $REPO_ROOT as "unixtime<TAB>iso8601" — used
# to order and date the Atom feed entries. Empty when history is unavailable.
git_first_commit() {
  git -C "$REPO_ROOT" log --format='%ct%x09%cI' --reverse -- "$1" 2>/dev/null | head -n1 || true
}

# blocks_* variants of the two helpers above, rooted at $BLOCKS_GIT_ROOT with
# paths relative to $BLOCKS_GIT_PREFIX rather than $REPO_ROOT — the blocks
# tree can live in a different git repo from $REPO_ROOT (post-split: a pinned
# checkout). $1 = path *inside* the blocks dir (e.g. "$slug/page"), or omitted
# for the blocks dir itself.
blocks_lastmod() {
  git -C "$BLOCKS_GIT_ROOT" log -1 --format=%cI -- "$BLOCKS_GIT_PREFIX${1:+/$1}" 2>/dev/null || true
}

blocks_first_commit() {
  git -C "$BLOCKS_GIT_ROOT" log --format='%ct%x09%cI' --reverse -- "$BLOCKS_GIT_PREFIX${1:+/$1}" 2>/dev/null | head -n1 || true
}

# Combined "unixtime iso8601" for a blocks path in one git invocation — used
# by the feed to compare <updated> across entries by unix time.
blocks_lastmod_ts() {
  git -C "$BLOCKS_GIT_ROOT" log -1 --format='%ct %cI' -- "$BLOCKS_GIT_PREFIX${1:+/$1}" 2>/dev/null || true
}

# Escape text for XML content and attribute values (&, <, >, quotes).
# sed, not ${var//…} — bash 5.2's patsub_replacement expands a bare `&` in
# the replacement to the matched text, silently corrupting the entities.
xml_escape() {
  printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' \
    -e 's/"/\&quot;/g' -e "s/'/\&apos;/g"
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
  blocks_mod="$(blocks_lastmod)"
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
    emit_url "$BASE_URL/tools/$slug/" "$(blocks_lastmod "$slug/page")"
  done
  # "X to Y" conversion pair pages (/tools/<parent>/<src>-to-<tgt>/) from the
  # generator's _pairs.json. Skipped gracefully when the file is missing
  # (generator not yet run); each pair page is generated from its parent
  # converter's page sources, so it shares that parent's lastmod.
  pairs_json="$PKG_DIR/tools/_pairs.json"
  if [[ -f "$pairs_json" ]]; then
    while IFS= read -r pair_path; do
      [[ -z "$pair_path" ]] && continue
      parent="${pair_path%%/*}"
      emit_url "$BASE_URL/tools/$pair_path/" "$(blocks_lastmod "$parent/page")"
    done < <(jq -r '.[].path' "$pairs_json")
  fi
  printf '</urlset>\n'
} > "$PKG_DIR/sitemap.xml"

# --- robots.txt ---
{
  printf 'User-agent: *\n'
  printf 'Allow: /\n'
  printf 'Sitemap: %s/sitemap.xml\n' "$BASE_URL"
} > "$PKG_DIR/robots.txt"

# --- feed.xml (Atom, the 50 newest tools by first commit of blocks/<slug>/page) ---
# Ordering/dating needs git history; when it is unavailable (shallow clone,
# no git) skip the feed with a warning rather than emitting undated entries.
feed_rows=()
for slug in "${page_slugs[@]}"; do
  first="$(blocks_first_commit "$slug/page")"
  [[ -z "$first" ]] && continue
  feed_rows+=("$first"$'\t'"$slug")
done
feed_written=""
if ((${#feed_rows[@]} == 0)); then
  echo "gen-seo.sh: warning: no git history for blocks/*/page — skipping feed.xml" >&2
else
  # Newest first by first-commit unix time; the top 50 become entries.
  mapfile -t feed_newest < <(printf '%s\n' "${feed_rows[@]}" | sort -t$'\t' -k1,1nr | head -n50)
  # Resolve each entry's <updated> (last commit, like sitemap <lastmod>) and
  # track the feed-level <updated> = max entry updated, compared by unix time
  # (ISO strings with mixed UTC offsets don't sort lexically).
  feed_entries=()
  feed_updated=""
  feed_updated_ts=0
  for row in "${feed_newest[@]}"; do
    IFS=$'\t' read -r _ published slug <<< "$row"
    read -r updated_ts updated <<< "$(blocks_lastmod_ts "$slug/page")"
    if [[ -z "$updated" ]]; then
      updated="$published"
      updated_ts=0
    fi
    if ((updated_ts > feed_updated_ts)); then
      feed_updated_ts=$updated_ts
      feed_updated="$updated"
    fi
    feed_entries+=("$published"$'\t'"$updated"$'\t'"$slug")
  done
  [[ -z "$feed_updated" ]] && feed_updated="${feed_entries[0]%%$'\t'*}"
  {
    printf '<?xml version="1.0" encoding="UTF-8"?>\n'
    printf '<feed xmlns="http://www.w3.org/2005/Atom">\n'
    printf '  <title>New tools — gizza.ai</title>\n'
    printf '  <id>%s/feed.xml</id>\n' "$BASE_URL"
    printf '  <link href="%s/feed.xml" rel="self" type="application/atom+xml"/>\n' "$BASE_URL"
    printf '  <link href="%s/tools/"/>\n' "$BASE_URL"
    printf '  <updated>%s</updated>\n' "$feed_updated"
    printf '  <author><name>gizza.ai</name></author>\n'
    for row in "${feed_entries[@]}"; do
      IFS=$'\t' read -r published updated slug <<< "$row"
      meta="$BLOCKS_DIR/$slug/page/meta.toml"
      title="$(meta_field "$meta" title)"
      [[ -z "$title" ]] && title="$slug"
      summary="$(meta_field "$meta" description)"
      [[ -z "$summary" ]] && summary="$title"
      url="$BASE_URL/tools/$slug/"
      printf '  <entry>\n'
      printf '    <id>%s</id>\n' "$url"
      printf '    <title>%s</title>\n' "$(xml_escape "$title")"
      printf '    <summary>%s</summary>\n' "$(xml_escape "$summary")"
      printf '    <link href="%s"/>\n' "$url"
      printf '    <published>%s</published>\n' "$published"
      printf '    <updated>%s</updated>\n' "$updated"
      printf '  </entry>\n'
    done
    printf '</feed>\n'
  } > "$PKG_DIR/feed.xml"
  feed_written=1
fi

# --- IndexNow key file (https://www.indexnow.org/) ---
# The key is public by design: search engines fetch <site>/<key>.txt to verify
# ownership before accepting pings. scripts/indexnow-ping.sh submits changed
# URLs with this same key after each deploy.
INDEXNOW_KEY="0d80c64b419d8ab46ad4b67e39d7d6c3"
printf '%s' "$INDEXNOW_KEY" > "$PKG_DIR/$INDEXNOW_KEY.txt"

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
        meta="$BLOCKS_DIR/$slug/page/meta.toml"
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
  printf -- '- [Atom feed](%s/feed.xml): the 50 newest tools, for feed readers and crawlers\n' "$BASE_URL"
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

echo "gen-seo.sh: wrote $PKG_DIR/sitemap.xml, $PKG_DIR/robots.txt${feed_written:+, $PKG_DIR/feed.xml}, $PKG_DIR/$INDEXNOW_KEY.txt, $PKG_DIR/llms.txt${index_mds[0]:+, $PKG_DIR/llms-full.txt}"
