#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cleanup() { rm -rf "$root/blocks/zzscratch-pure" "$root/blocks/zzscratch-ff"; }
trap cleanup EXIT
"$root/scripts/scaffold-tool.sh" zzscratch-pure pure
"$root/scripts/scaffold-tool.sh" zzscratch-ff ffmpeg

# Both scaffolds pin wafer-run at wafer-run-pin.txt, spelled exactly as
# scripts/set-wafer-run-rev.sh writes it, and never on a branch.
pin="$(tr -d '[:space:]' < "$root/wafer-run-pin.txt")"
want_sdk="wafer-sdk = { git = \"https://github.com/wafer-run/wafer-run\", rev = \"$pin\" }"
want_block="wafer-block = { git = \"https://github.com/wafer-run/wafer-run\", rev = \"$pin\" }"
for slug in zzscratch-pure zzscratch-ff; do
  manifest="$root/blocks/$slug/Cargo.toml"
  grep -qxF "$want_sdk" "$manifest" || { echo "FAIL: $manifest lacks: $want_sdk" >&2; exit 1; }
  grep -qxF "$want_block" "$manifest" || { echo "FAIL: $manifest lacks: $want_block" >&2; exit 1; }
  if grep -n 'branch *=' "$root/blocks/$slug/Cargo.toml" "$root/blocks/$slug/core/Cargo.toml" "$root/blocks/$slug/web/Cargo.toml"; then
    echo "FAIL: blocks/$slug pins a dependency to a branch" >&2
    exit 1
  fi
done

(cd "$root/blocks/zzscratch-pure" && cargo check --workspace)
(cd "$root/blocks/zzscratch-ff" && cargo check --workspace)

# The first resolution lands on the pin: one wafer-run source, at the pinned rev.
want_source="source = \"git+https://github.com/wafer-run/wafer-run?rev=$pin#$pin\""
for slug in zzscratch-pure zzscratch-ff; do
  sources="$(grep -F 'github.com/wafer-run/wafer-run' "$root/blocks/$slug/Cargo.lock" | sort -u)"
  [ "$sources" = "$want_source" ] || {
    echo "FAIL: blocks/$slug/Cargo.lock resolves wafer-run from:" >&2
    echo "$sources" >&2
    exit 1
  }
done
echo "scaffold-tool.test.sh OK"
