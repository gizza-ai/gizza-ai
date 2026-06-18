#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

# Create a temp dir that mirrors the repo structure expected by gen-seo.sh
tmpdir="$(mktemp -d)"
cleanup() { rm -rf "$tmpdir"; }
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

# Create pkg/ dir
mkdir -p "$tmpdir/pkg"

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
if ! grep -qF "https://example.test/tools/calculator/" "$sitemap"; then
  echo "FAIL: sitemap missing calculator tool URL"
  exit 1
fi
if grep -qF "web-fetch" "$sitemap"; then
  echo "FAIL: sitemap must NOT contain web-fetch (no page)"
  exit 1
fi

# --- robots.txt assertions ---
robots="$tmpdir/pkg/robots.txt"
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

echo "gen-seo.test.sh OK"
