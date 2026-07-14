#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

# Create a temp dir that mirrors the repo structure expected by gen-seo.sh
tmpdir="$(mktemp -d)"
cleanup() { rm -rf "$tmpdir" "${tmpdir2:-}"; }
trap cleanup EXIT

# Create the stub gizza binary
mkdir -p "$tmpdir/bin"
cat > "$tmpdir/bin/gizza" <<'STUB'
#!/usr/bin/env bash
if [[ "${1:-}" == "list" && "${2:-}" == "--json-out" ]]; then
  printf '[{"name":"gizza-ai/calculator","description":"A simple calculator","parameters":[]},{"name":"gizza-ai/web-fetch","description":"Fetch a web page and return its content","parameters":[]}]\n'
  exit 0
fi
echo "stub gizza: unknown args" >&2
exit 1
STUB
chmod +x "$tmpdir/bin/gizza"

# Create fake blocks/calculator/page/meta.toml (has a page)
mkdir -p "$tmpdir/blocks/calculator/page"
printf 'title = "Calculator"\n' > "$tmpdir/blocks/calculator/page/meta.toml"

# No blocks/web-fetch/page/meta.toml (chat-only tool)

# Create a fake blocks/page tool that is NOT returned by the stub gizza list
# (simulates the CLI catalog lagging behind the on-disk blocks/*/page tools)
mkdir -p "$tmpdir/blocks/ghost-tool/page"
printf 'title = "Ghost Tool"\ndescription = "A tool only present on disk"\n' > "$tmpdir/blocks/ghost-tool/page/meta.toml"

# Create pkg/ dir
mkdir -p "$tmpdir/pkg"

# Pair pages: the generator writes pkg/tools/_pairs.json; gen-seo.sh adds each
# path to the sitemap. (calculator stands in for a converter parent here.)
mkdir -p "$tmpdir/pkg/tools"
printf '[{"path":"calculator/wav-to-mp3","title":"Convert WAV to MP3"}]\n' \
  > "$tmpdir/pkg/tools/_pairs.json"

# Run gen-seo.sh from the temp dir (as repo root)
BASE_URL="https://example.test" \
GIZZA="$tmpdir/bin/gizza" \
  bash "$root/scripts/gen-seo.sh" "$tmpdir"

# --- sitemap.xml assertions ---
sitemap="$tmpdir/pkg/sitemap.xml"
if ! grep -qF "https://example.test/" "$sitemap"; then
  echo "FAIL: sitemap missing apex URL https://example.test/"
  exit 1
fi
if ! grep -qF "https://example.test/chat" "$sitemap"; then
  echo "FAIL: sitemap missing chat URL https://example.test/chat"
  exit 1
fi
if ! grep -qF "https://example.test/tools/" "$sitemap"; then
  echo "FAIL: sitemap missing tools index URL https://example.test/tools/"
  exit 1
fi
if ! grep -qF "https://example.test/tools/calculator/" "$sitemap"; then
  echo "FAIL: sitemap missing calculator tool URL"
  exit 1
fi
if grep -qF "web-fetch" "$sitemap"; then
  echo "FAIL: sitemap must NOT contain web-fetch (no page)"
  exit 1
fi
# ghost-tool has a blocks/*/page/meta.toml but is absent from gizza list —
# it must still appear in the sitemap (derived directly from the filesystem)
if ! grep -qF "https://example.test/tools/ghost-tool/" "$sitemap"; then
  echo "FAIL: sitemap missing ghost-tool page URL (page exists but not in gizza list)"
  exit 1
fi
# conversion pair pages from pkg/tools/_pairs.json land in the sitemap
if ! grep -qF "https://example.test/tools/calculator/wav-to-mp3/" "$sitemap"; then
  echo "FAIL: sitemap missing pair page URL from _pairs.json"
  exit 1
fi

# --- robots.txt assertions ---
robots="$tmpdir/pkg/robots.txt"
if ! grep -qF "User-agent: *" "$robots"; then
  echo "FAIL: robots.txt missing 'User-agent: *' line"
  exit 1
fi
if ! grep -qF "Allow: /" "$robots"; then
  echo "FAIL: robots.txt missing 'Allow: /' line"
  exit 1
fi
if ! grep -qF "Sitemap: https://example.test/sitemap.xml" "$robots"; then
  echo "FAIL: robots.txt missing Sitemap: line"
  exit 1
fi

# --- llms.txt assertions ---
llmstxt="$tmpdir/pkg/llms.txt"
if ! grep -qF "# gizza.ai" "$llmstxt"; then
  echo "FAIL: llms.txt missing '# gizza.ai' heading"
  exit 1
fi
if ! grep -qF "## Tools" "$llmstxt"; then
  echo "FAIL: llms.txt missing '## Tools' heading"
  exit 1
fi
if ! grep -qF "> " "$llmstxt"; then
  echo "FAIL: llms.txt missing summary blockquote line (starting with '> ')"
  exit 1
fi
if ! grep -qF "https://example.test/tools/calculator/index.md" "$llmstxt"; then
  echo "FAIL: llms.txt missing calculator index.md link"
  exit 1
fi
if ! grep -qF "Fetch a web page and return its content" "$llmstxt"; then
  echo "FAIL: llms.txt missing web-fetch description line"
  exit 1
fi
if grep -qF "https://example.test/tools/web-fetch/" "$llmstxt"; then
  echo "FAIL: llms.txt must NOT contain a web-fetch page link"
  exit 1
fi
# ghost-tool: page tool absent from gizza JSON must still get a linked entry,
# with its description sourced from page/meta.toml as a fallback
if ! grep -qF "https://example.test/tools/ghost-tool/index.md" "$llmstxt"; then
  echo "FAIL: llms.txt missing ghost-tool index.md link (page exists but not in gizza list)"
  exit 1
fi
if ! grep -qF "A tool only present on disk" "$llmstxt"; then
  echo "FAIL: llms.txt missing ghost-tool fallback description from meta.toml"
  exit 1
fi


# --- BLOCKS_DIR in a SEPARATE git repo (post-split site build) ---
# Regression test: the blocks tree's git history lives in whatever repo
# contains $BLOCKS_DIR, which post-split is a different repo from $REPO_ROOT
# (e.g. BLOCKS_DIR=gizza-ai/blocks, a pinned checkout with its own .git).
# git-history lookups that hardcode `-C "$REPO_ROOT"` silently return nothing
# in that layout, and sitemap <lastmod>/feed.xml go empty.
tmpdir2="$(mktemp -d)"
repo_root2="$tmpdir2/site-repo"
blocks_repo="$tmpdir2/blocks-repo"
mkdir -p "$repo_root2/pkg" "$blocks_repo/blocks/foreign-tool/page"

printf 'title = "Foreign Tool"\ndescription = "Lives in a separate git repo"\n' \
  > "$blocks_repo/blocks/foreign-tool/page/meta.toml"

git -C "$blocks_repo" init -q
git -C "$blocks_repo" add -A
git -C "$blocks_repo" \
  -c user.email=test@example.com -c user.name=test \
  commit -q -m "add foreign-tool"

BASE_URL="https://example.test" \
GIZZA="$tmpdir/bin/gizza" \
BLOCKS_DIR="$blocks_repo/blocks" \
  bash "$root/scripts/gen-seo.sh" "$repo_root2"

foreign_sitemap="$repo_root2/pkg/sitemap.xml"
if ! grep -qF "https://example.test/tools/foreign-tool/" "$foreign_sitemap"; then
  echo "FAIL: foreign-repo sitemap missing foreign-tool URL"
  exit 1
fi
if ! grep -A1 "https://example.test/tools/foreign-tool/</loc>" "$foreign_sitemap" | grep -qF "<lastmod>"; then
  echo "FAIL: foreign-repo sitemap missing <lastmod> for foreign-tool (BLOCKS_DIR git history lookup did not follow BLOCKS_DIR into its own repo)"
  exit 1
fi

echo "gen-seo.test.sh OK"
