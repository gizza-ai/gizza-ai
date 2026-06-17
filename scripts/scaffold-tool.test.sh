#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cleanup() { rm -rf "$root/blocks/zzscratch-pure" "$root/blocks/zzscratch-ff"; }
trap cleanup EXIT
"$root/scripts/scaffold-tool.sh" zzscratch-pure pure
"$root/scripts/scaffold-tool.sh" zzscratch-ff ffmpeg
(cd "$root/blocks/zzscratch-pure" && cargo check --workspace)
(cd "$root/blocks/zzscratch-ff" && cargo check --workspace)
echo "scaffold-tool.test.sh OK"
