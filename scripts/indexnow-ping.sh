#!/usr/bin/env bash
# indexnow-ping.sh — submit URLs changed in a git commit range to IndexNow.
# Usage: RANGE=abc123..def456 scripts/indexnow-ping.sh [--dry-run]
#    or: scripts/indexnow-ping.sh [--dry-run] abc123..def456
#
# Maps changed paths to public URLs (blocks/<slug>/page/** → the tool page,
# site/** → the homepage) and POSTs them to https://api.indexnow.org/indexnow,
# which fans out to all IndexNow-capable search engines. The key must match
# the INDEXNOW_KEY constant in gen-seo.sh (which publishes pkg/<key>.txt).
#
# Never fails the caller: an invalid range (e.g. force-push zeros), an empty
# URL list, or a network/HTTP error all exit 0 — a missed ping must not break
# a deploy. Search engines re-crawl from the sitemap regardless.
set -euo pipefail

BASE_URL="${BASE_URL:-https://gizza.ai}"
HOST="${BASE_URL#https://}"
HOST="${HOST#http://}"
ENDPOINT="https://api.indexnow.org/indexnow"
# Public by design — must match gen-seo.sh, which serves it at /<key>.txt.
INDEXNOW_KEY="0d80c64b419d8ab46ad4b67e39d7d6c3"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

DRY_RUN=""
RANGE="${RANGE:-}"
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    *) RANGE="$arg" ;;
  esac
done

if [[ -z "$RANGE" ]]; then
  echo "indexnow-ping.sh: no commit range given (env RANGE or \$1) — nothing to do" >&2
  exit 0
fi

# Invalid range (force-push zeros, unknown SHAs, shallow history): warn, exit 0.
if ! changed="$(git -C "$REPO_ROOT" diff --name-only "$RANGE" -- 2>/dev/null)"; then
  echo "indexnow-ping.sh: warning: cannot resolve range '$RANGE' — skipping ping" >&2
  exit 0
fi

# Map changed paths to public URLs, deduped, capped at 500.
urls=()
seen=$'\n'
add_url() {
  [[ "$seen" == *$'\n'"$1"$'\n'* ]] && return 0
  seen+="$1"$'\n'
  urls+=("$1")
}
while IFS= read -r path; do
  [[ -z "$path" ]] && continue
  case "$path" in
    blocks/*/page/*)
      slug="${path#blocks/}"
      slug="${slug%%/*}"
      add_url "$BASE_URL/tools/$slug/"
      ;;
    site/*)
      add_url "$BASE_URL/"
      ;;
  esac
done <<< "$changed"

((${#urls[@]} == 0)) && exit 0
((${#urls[@]} > 500)) && urls=("${urls[@]:0:500}")

payload="$(
  jq -n \
    --arg host "$HOST" \
    --arg key "$INDEXNOW_KEY" \
    --arg keyLocation "$BASE_URL/$INDEXNOW_KEY.txt" \
    '{host: $host, key: $key, keyLocation: $keyLocation, urlList: $ARGS.positional}' \
    --args "${urls[@]}"
)"

if [[ -n "$DRY_RUN" ]]; then
  echo "indexnow-ping.sh: dry run — would POST to $ENDPOINT:"
  printf '%s\n' "$payload"
  exit 0
fi

if status="$(
  curl -sS --max-time 30 -o /dev/null -w '%{http_code}' -X POST "$ENDPOINT" \
    -H 'Content-Type: application/json; charset=utf-8' \
    --data "$payload"
)"; then
  case "$status" in
    200 | 202) echo "indexnow-ping.sh: submitted ${#urls[@]} URL(s) (HTTP $status)" ;;
    *) echo "indexnow-ping.sh: warning: IndexNow returned HTTP $status for ${#urls[@]} URL(s)" >&2 ;;
  esac
else
  echo "indexnow-ping.sh: warning: POST to $ENDPOINT failed — skipping ping" >&2
fi
exit 0
