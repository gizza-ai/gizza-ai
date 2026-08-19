#!/usr/bin/env bash
# Print the blocks the "Refresh block WASM" workflow should rebuild, one per
# line. Either the explicit INPUT_BLOCKS list, or every block whose compiled
# output could have changed relative to main — the same path filter the
# "Detect changed blocks" step in test.yml applies.
set -euo pipefail

if [ -n "${INPUT_BLOCKS:-}" ]; then
  printf '%s\n' ${INPUT_BLOCKS} | sed '/^$/d' | sort -u
  exit 0
fi

git fetch --no-tags origin main >/dev/null 2>&1 || true
base="$(git merge-base origin/main HEAD)"
git diff --name-only "$base...HEAD" \
  | grep -vE '^blocks/[^/]*/(web/pkg/|page/|manifest\.json$)' \
  | sed -n 's#^blocks/\([^/]*\)/.*#\1#p' \
  | sort -u
