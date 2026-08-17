#!/usr/bin/env bash
# Build the canonical CLI/MCP WASM artifact for one block.
#
# Usage:
#   scripts/build-block-wasm.sh <slug>          # refresh Cargo.lock + target/block.wasm
#   scripts/build-block-wasm.sh <slug> --check  # rebuild with the committed lock and compare
set -euo pipefail

slug="${1:?usage: build-block-wasm.sh <slug> [--check]}"
mode="${2:-write}"
[[ "$slug" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ ]] || {
  echo "slug must be kebab-case [a-z0-9-], no leading/trailing hyphen" >&2
  exit 2
}
[[ "$mode" == "write" || "$mode" == "--check" ]] || {
  echo "usage: build-block-wasm.sh <slug> [--check]" >&2
  exit 2
}

root="$(cd "$(dirname "$0")/.." && pwd)"
block_dir="$root/blocks/$slug"
canonical_root="/tmp/gizza-ai-canonical-wasm-workspace"
canonical_block_dir="$canonical_root/blocks/$slug"
canonical_cargo_home="/tmp/gizza-ai-canonical-cargo-home"
canonical_rust_sysroot="/tmp/gizza-ai-canonical-rust-toolchain"
artifact="$block_dir/target/block.wasm"
lockfile="$block_dir/Cargo.lock"
pin="$(tr -d '[:space:]' < "$root/wafer-run-pin.txt")"
expected="gizza_ai_${slug//-/_}_block.wasm"

[[ -f "$block_dir/Cargo.toml" ]] || {
  echo "blocks/$slug/Cargo.toml not found" >&2
  exit 2
}
command -v cargo >/dev/null || {
  echo "cargo is required" >&2
  exit 2
}
command -v jq >/dev/null || {
  echo "jq is required to locate Cargo's WASM artifact" >&2
  exit 2
}
command -v rustc >/dev/null || {
  echo "rustc is required" >&2
  exit 2
}
command -v realpath >/dev/null || {
  echo "realpath is required to validate the canonical build directories" >&2
  exit 2
}
command -v rsync >/dev/null || {
  echo "rsync is required to stage canonical WASM sources" >&2
  exit 2
}

# Cargo hashes absolute path-dependency identities before invoking rustc. Stage
# the block and shared utility crate at a fixed path so those identities match
# across local and CI builds. Use fixed paths for registry/git sources and the
# pinned Rust sysroot, remap those roots, and strip non-runtime symbol metadata.
actual_rust_sysroot="$(rustc --print sysroot)"
[[ -d "$actual_rust_sysroot/lib/rustlib/src/rust/library" ]] || {
  echo "::error::the pinned rust-src component is required for canonical source paths." >&2
  echo "Run: rustup component add rust-src --toolchain 1.94.0" >&2
  exit 1
}
canonical_rustflags=(
  "-Ccodegen-units=1"
  "-Cstrip=symbols"
  "--sysroot=$canonical_rust_sysroot"
  "--remap-path-prefix=$canonical_cargo_home=/cargo-home"
  "--remap-path-prefix=$canonical_rust_sysroot=/rust-toolchain"
  "--remap-path-prefix=$canonical_root=/workspace"
)
canonical_encoded_rustflags="$(printf '%s\x1f' "${canonical_rustflags[@]}")"
canonical_encoded_rustflags="${canonical_encoded_rustflags%$'\x1f'}"

if [[ "$mode" == "--check" ]]; then
  git -C "$root" ls-files --error-unmatch "blocks/$slug/Cargo.lock" >/dev/null 2>&1 || {
    echo "::error::blocks/$slug/Cargo.lock must be committed for reproducible WASM builds." >&2
    echo "Run scripts/build-block-wasm.sh $slug, then stage Cargo.lock and target/block.wasm." >&2
    exit 1
  }
  git -C "$root" ls-files --error-unmatch "blocks/$slug/target/block.wasm" >/dev/null 2>&1 || {
    echo "::error::blocks/$slug/target/block.wasm must be committed for CLI/MCP discovery." >&2
    echo "Run scripts/build-block-wasm.sh $slug, then stage Cargo.lock and target/block.wasm." >&2
    exit 1
  }
else
  (
    cd "$block_dir"
    [[ -f Cargo.lock ]] || cargo generate-lockfile
    # Keep the three Wafer crates aligned with the runtime pin. Some blocks do
    # not depend on every crate directly, so a missing package is harmless.
    for crate in wafer-sdk wafer-block wafer-block-macro; do
      cargo update -p "$crate" --precise "$pin" 2>/dev/null || true
    done
  )
fi

[[ -f "$lockfile" ]] || {
  echo "::error::blocks/$slug/Cargo.lock is missing." >&2
  exit 1
}

stray="$(
  grep -o 'github.com/wafer-run/wafer-run[^"]*#[0-9a-f]*' "$lockfile" \
    | sed 's/.*#//' \
    | sort -u \
    | grep -v "^$pin\$" || true
)"
if [[ -n "$stray" ]]; then
  echo "::error::blocks/$slug/Cargo.lock resolves wafer-run at '$stray'; expected '$pin'." >&2
  if [[ "$mode" == "--check" ]]; then
    echo "Run scripts/build-block-wasm.sh $slug to refresh the canonical artifact." >&2
  fi
  exit 1
fi

for canonical_dir in "$canonical_root" "$canonical_cargo_home"; do
  if [[ -L "$canonical_dir" || ( -e "$canonical_dir" && ! -d "$canonical_dir" ) ]]; then
    echo "::error::$canonical_dir must be a real directory, not a file or symlink." >&2
    exit 1
  fi
  mkdir -p "$canonical_dir"
  [[ "$(realpath "$canonical_dir")" == "$canonical_dir" ]] || {
    echo "::error::$canonical_dir resolves outside the canonical build location." >&2
    exit 1
  }
done
if [[ -L "$canonical_rust_sysroot" ]]; then
  resolved_rust_sysroot="$(realpath "$canonical_rust_sysroot" 2>/dev/null || true)"
  [[ "$resolved_rust_sysroot" == "$actual_rust_sysroot" ]] || {
    echo "::error::$canonical_rust_sysroot points at an unexpected Rust toolchain." >&2
    exit 1
  }
elif [[ -e "$canonical_rust_sysroot" ]]; then
  echo "::error::$canonical_rust_sysroot must be a symlink to the pinned Rust toolchain." >&2
  exit 1
else
  ln -s "$actual_rust_sysroot" "$canonical_rust_sysroot"
fi
rsync -a "$root/rust-toolchain.toml" "$canonical_root/rust-toolchain.toml"

# Every crate the block reaches through a relative `path = "../…"` dependency
# has to be staged as well: the shared block-utils crate, plus any sibling
# block's core a tool reuses so two tools share one implementation. A fresh CI
# runner starts with an empty canonical workspace, so a missing sibling fails
# the build outright instead of silently picking up a stale copy.
path_dep_dirs() {
  local dir="$1" manifest rel resolved
  find "$dir" -name Cargo.toml -not -path '*/target/*' -print | while IFS= read -r manifest; do
    grep -hoE 'path[[:space:]]*=[[:space:]]*"[^"]+"' "$manifest" \
      | sed -E 's/.*"([^"]+)"/\1/' \
      | while IFS= read -r rel; do
          resolved="$(realpath -m "$(dirname "$manifest")/$rel")"
          case "$resolved" in
            "$root"/*) resolved="${resolved#"$root/"}" ;;
            *) continue ;;
          esac
          case "$resolved" in
            blocks/*) printf '%s' "$resolved" | cut -d/ -f1-2 ;;
            *) printf '%s\n' "${resolved%%/*}" ;;
          esac
        done
  done
}

relatives="block-utils blocks/$slug"
queue="$relatives"
while [[ -n "$queue" ]]; do
  discovered=""
  for staged in $queue; do
    for candidate in $(path_dep_dirs "$root/$staged" | sort -u); do
      [[ -d "$root/$candidate" ]] || continue
      case " $relatives $discovered " in *" $candidate "*) continue ;; esac
      discovered="$discovered $candidate"
    done
  done
  relatives="$relatives$discovered"
  queue="$discovered"
done

for relative in $relatives; do
  source_dir="$root/$relative"
  canonical_dir="$canonical_root/$relative"
  if [[ -L "$canonical_dir" || ( -e "$canonical_dir" && ! -d "$canonical_dir" ) ]]; then
    echo "::error::$canonical_dir must be a real directory, not a file or symlink." >&2
    exit 1
  fi
  mkdir -p "$canonical_dir"
  [[ "$(realpath "$canonical_dir")" == "$canonical_dir" ]] || {
    echo "::error::$canonical_dir resolves outside the canonical build location." >&2
    exit 1
  }
  rsync -a --delete --exclude target "$source_dir/" "$canonical_dir/"
done

canonical_target="$(mktemp -d /tmp/gizza-ai-canonical-wasm-target.XXXXXX)"
build_json="$(mktemp)"
trap 'rm -f "$build_json"; rm -rf "$canonical_target"' EXIT
(
  cd "$canonical_block_dir"
  CARGO_HOME="$canonical_cargo_home" CARGO_TARGET_DIR="$canonical_target" \
    CARGO_ENCODED_RUSTFLAGS="$canonical_encoded_rustflags" \
    cargo build --locked --target wasm32-wasip1 --release --message-format=json > "$build_json"
)
wasm="$(
  jq -r 'select(.reason == "compiler-artifact") | .filenames[]? | select(endswith(".wasm"))' \
    "$build_json" \
    | while IFS= read -r candidate; do
        if [[ "$(basename "$candidate")" == "$expected" ]]; then
          printf '%s\n' "$candidate"
        fi
      done \
    | tail -n 1
)"
[[ -n "$wasm" && -f "$wasm" ]] || {
  echo "::error::Cargo did not produce the expected $expected for blocks/$slug." >&2
  exit 1
}

if [[ "$mode" == "--check" ]]; then
  [[ -f "$artifact" ]] || {
    echo "::error::$artifact is missing." >&2
    exit 1
  }
  if ! cmp -s "$wasm" "$artifact"; then
    echo "::error::blocks/$slug/target/block.wasm does not match the canonical locked build." >&2
    echo "committed: $(sha256sum "$artifact" | cut -d' ' -f1) ($(stat -c '%s' "$artifact") bytes)" >&2
    echo "rebuilt:   $(sha256sum "$wasm" | cut -d' ' -f1) ($(stat -c '%s' "$wasm") bytes)" >&2
    echo "rustc: $(rustc --version --verbose | tr '\n' ' ')" >&2
    echo "first byte differences (position, committed, rebuilt; octal):" >&2
    cmp -l "$artifact" "$wasm" | head -n 20 >&2 || true
    if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
      mismatch_dir="/tmp/gizza-ai-mismatched-wasm"
      mkdir -p "$mismatch_dir"
      cp "$wasm" "$mismatch_dir/$slug.wasm"
    fi
    echo "Run scripts/build-block-wasm.sh $slug and commit the refreshed artifact." >&2
    exit 1
  fi
  echo "verified blocks/$slug/target/block.wasm"
else
  mkdir -p "$(dirname "$artifact")"
  cp "$wasm" "$artifact"
  echo "wrote blocks/$slug/Cargo.lock"
  echo "wrote blocks/$slug/target/block.wasm"
  echo "stage both ignored artifacts with:"
  echo "  git add -f blocks/$slug/Cargo.lock blocks/$slug/target/block.wasm"
fi
