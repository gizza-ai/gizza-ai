#!/usr/bin/env bash
# Self-test for scripts/set-wafer-run-rev.sh. Builds a throwaway repo with every
# wafer-run dependency spelling the corpus has used (branch = "main", the `.git`
# URL, a short rev, a tag, extra keys, dev and target-specific tables), pins it,
# and checks that the rewrite keeps every other key and that --check refuses
# each way the pin can break: a branch, a short or mis-spelled rev, a bad pin
# file, a lock at another rev, and a lock holding two copies of wafer-block.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM

old=914a0aad82a5e6af95a124de6ad841943499a5ad
sha=f6132178467f2d93ce02a625e3220e30cfa278ee
url=https://github.com/wafer-run/wafer-run
failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

# expect_check <description> <ok|fail> [pattern the output must contain]
expect_check() {
  local what="$1" want="$2" pattern="${3:-}" out status=0
  out="$(cd "$fx" && scripts/set-wafer-run-rev.sh --check 2>&1)" || status=$?
  if [ "$want" = ok ] && [ "$status" -ne 0 ]; then
    fail "$what: --check failed but should pass"
    echo "$out" >&2
  elif [ "$want" = fail ] && [ "$status" -eq 0 ]; then
    fail "$what: --check passed but should fail"
  elif [ -n "$pattern" ] && ! grep -qF -- "$pattern" <<<"$out"; then
    fail "$what: --check output lacks '$pattern'"
    echo "$out" >&2
  else
    echo "ok: $what"
  fi
}

lock_entry() { # <name> <source-line or empty>
  printf '[[package]]\nname = "%s"\nversion = "0.1.0"\n' "$1"
  [ -z "$2" ] || printf 'source = "%s"\n' "$2"
  printf '\n'
}

fx="$work/repo"
mkdir -p "$fx/scripts" "$fx/cli" "$fx/block-utils" "$fx/blocks/alpha/core" "$fx/blocks/beta" \
  "$fx/blocks/alpha/target/sub" "$fx/.claude/worktrees/other/blocks/x"
cp "$root/scripts/set-wafer-run-rev.sh" "$fx/scripts/"
echo "$old" > "$fx/wafer-run-pin.txt"

cat > "$fx/cli/Cargo.toml" <<EOF
[package]
name = "cli"
version = "0.1.0"

[dependencies]
wafer-run = { git = "$url", branch = "main", default-features = false, features = ["wasm", "x"] } # runtime
wafer-core = { git = "$url", rev = "914a0aad" }
"wafer-block-network" = { git = "$url.git", rev = "$old" }
other-git = { git = "https://example.com/other/repo", rev = "0123456789abcdef0123456789abcdef01234567" }
serde = { version = "1", features = ["derive"] }
EOF

cat > "$fx/block-utils/Cargo.toml" <<EOF
[package]
name = "block-utils"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url.git", branch = "main" }
wafer-block = { git = "$url.git", branch = "main" }
EOF

cat > "$fx/blocks/alpha/Cargo.toml" <<EOF
[workspace]
members = [".", "core"]

[package]
name = "alpha"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url.git", branch = "main" }
renamed = { package = "wafer-block", git = "$url/", tag = "v0.1.0", optional = true }
alpha-core = { path = "core" }

[dev-dependencies]
wafer-block = { git = "HTTPS://GitHub.com/wafer-run/wafer-run.git", branch = "main" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wafer-block-macro = { git = "$url", rev = "$old", features = ["a", "b,c"] }
EOF

cat > "$fx/blocks/alpha/core/Cargo.toml" <<'EOF'
[package]
name = "alpha-core"
version = "0.1.0"

[dependencies]
EOF

cat > "$fx/blocks/beta/Cargo.toml" <<EOF
[package]
name = "beta"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url", rev = "$old" }
EOF

# Not ours to touch: build output and other checkouts in dot-directories.
for d in "$fx/blocks/alpha/target/sub" "$fx/.claude/worktrees/other/blocks/x"; do
  printf '[package]\nname = "x"\nversion = "0.1.0"\n[dependencies]\nwafer-sdk = { git = "%s", branch = "main" }\n' "$url" > "$d/Cargo.toml"
done

git -C "$fx" init -q
git -C "$fx" add -A

# --- argument validation ----------------------------------------------------
for bad in 914a0aad "${sha^^}" "${sha}0" "g${sha:1}" ""; do
  if (cd "$fx" && scripts/set-wafer-run-rev.sh "$bad" >/dev/null 2>&1); then
    fail "set-wafer-run-rev.sh accepted the sha '$bad'"
  else
    echo "ok: refuses sha '$bad'"
  fi
done
git -C "$fx" diff --quiet || fail "a refused sha still modified files"

expect_check "unpinned corpus is refused" fail 'uses branch = '"'main'"

# --- rewrite ----------------------------------------------------------------
(cd "$fx" && scripts/set-wafer-run-rev.sh "$sha" >/dev/null) || fail "set-wafer-run-rev.sh $sha failed"

[ "$(cat "$fx/wafer-run-pin.txt")" = "$sha" ] || fail "wafer-run-pin.txt was not written"

expected_cli="$(cat <<EOF
[package]
name = "cli"
version = "0.1.0"

[dependencies]
wafer-run = { git = "$url", rev = "$sha", default-features = false, features = ["wasm", "x"] } # runtime
wafer-core = { git = "$url", rev = "$sha" }
"wafer-block-network" = { git = "$url", rev = "$sha" }
other-git = { git = "https://example.com/other/repo", rev = "0123456789abcdef0123456789abcdef01234567" }
serde = { version = "1", features = ["derive"] }
EOF
)"
[ "$(cat "$fx/cli/Cargo.toml")" = "$expected_cli" ] || {
  fail "cli/Cargo.toml rewrite differs from the expected text"
  diff <(echo "$expected_cli") "$fx/cli/Cargo.toml" >&2 || true
}

expected_alpha="$(cat <<EOF
[workspace]
members = [".", "core"]

[package]
name = "alpha"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url", rev = "$sha" }
renamed = { git = "$url", rev = "$sha", package = "wafer-block", optional = true }
alpha-core = { path = "core" }

[dev-dependencies]
wafer-block = { git = "$url", rev = "$sha" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wafer-block-macro = { git = "$url", rev = "$sha", features = ["a", "b,c"] }
EOF
)"
[ "$(cat "$fx/blocks/alpha/Cargo.toml")" = "$expected_alpha" ] || {
  fail "blocks/alpha/Cargo.toml rewrite differs from the expected text"
  diff <(echo "$expected_alpha") "$fx/blocks/alpha/Cargo.toml" >&2 || true
}

git -C "$fx" diff --quiet -- blocks/alpha/core/Cargo.toml blocks/alpha/target .claude \
  || fail "the rewrite touched a manifest it must leave alone"
grep -q 'branch = "main"' "$fx/blocks/alpha/target/sub/Cargo.toml" \
  || fail "the rewrite touched target/"

git -C "$fx" add -A
(cd "$fx" && scripts/set-wafer-run-rev.sh "$sha" >/dev/null)
if git -C "$fx" diff --quiet; then
  echo "ok: a second run changes nothing"
else
  fail "a second run with the same sha changed files"
fi

# --- check: locks -----------------------------------------------------------
good="git+$url?rev=$sha#$sha"
write_locks() { # <source used for wafer-block in blocks/beta>
  { lock_entry wafer-run "$good"; lock_entry wafer-block "$good"; lock_entry serde ""; } > "$fx/cli/Cargo.lock"
  { lock_entry wafer-sdk "$good"; lock_entry wafer-block "$good"; } > "$fx/block-utils/Cargo.lock"
  { lock_entry wafer-sdk "$good"; lock_entry wafer-block "$1"; lock_entry wafer-block-macro "$good"; } > "$fx/blocks/beta/Cargo.lock"
  git -C "$fx" add -f cli/Cargo.lock block-utils/Cargo.lock blocks/beta/Cargo.lock
}
write_locks "$good"
expect_check "pinned manifests and locks pass" ok

write_locks "git+$url?branch=main#$old"
expect_check "lock resolving wafer-run on a branch" fail "blocks/beta/Cargo.lock resolves wafer-run from git+$url?branch=main#$old"

write_locks "$good"
lock_entry wafer-block "git+$url?branch=main#$sha" >> "$fx/blocks/beta/Cargo.lock"
git -C "$fx" add -f blocks/beta/Cargo.lock
expect_check "lock with two wafer-block packages" fail "blocks/beta/Cargo.lock holds 2 copies of wafer-block"

write_locks "$good"
{ lock_entry wafer-sdk "$good"; lock_entry wafer-sdk "git+$url?rev=$sha#$sha"; } > "$fx/block-utils/Cargo.lock"
git -C "$fx" add -f block-utils/Cargo.lock
expect_check "lock with two wafer-sdk packages from one source" fail "block-utils/Cargo.lock holds 2 copies of wafer-sdk"

write_locks "$good"
lock_entry serde "" > "$fx/blocks/beta/Cargo.lock"
git -C "$fx" add -f blocks/beta/Cargo.lock
expect_check "block lock that never resolves wafer-run" fail "blocks/beta/Cargo.lock does not resolve wafer-run at all"

write_locks "$good"
expect_check "locks restored" ok

# --- check: manifests and pin file ------------------------------------------
mutate() { # <file> <sed expression>; the original is restored by restore
  cp "$fx/$1" "$work/saved"
  saved_path="$1"
  sed "$2" "$work/saved" > "$fx/$1"
}
restore() { cp "$work/saved" "$fx/$saved_path"; }

mutate blocks/beta/Cargo.toml "s|rev = \"$sha\"|branch = \"main\"|"
expect_check 'block manifest on branch = "main"' fail "blocks/beta/Cargo.toml: dependencies.wafer-sdk uses branch = 'main'"
restore

mutate blocks/beta/Cargo.toml "s|rev = \"$sha\"|rev = \"$sha\", branch = \"main\"|"
expect_check "rev and branch together" fail "uses branch = 'main'"
restore

mutate cli/Cargo.toml "s|rev = \"$sha\" }$|rev = \"${sha:0:8}\" }|"
expect_check "short rev" fail "pins wafer-run at rev '${sha:0:8}'"
restore

mutate blocks/beta/Cargo.toml "s|$url\"|$url.git\"|"
expect_check "the .git URL spelling" fail "spells the wafer-run URL '$url.git'"
restore

mutate cli/Cargo.toml 's|other/repo", rev = "[0-9a-f]*"|other/repo", branch = "dev"|'
expect_check "a non-wafer-run git dependency on a branch" fail "dependencies.other-git uses branch = 'dev'"
restore

cp "$fx/wafer-run-pin.txt" "$work/pin"
echo "${sha:0:12}" > "$fx/wafer-run-pin.txt"
expect_check "short sha in wafer-run-pin.txt" fail "not a full 40-character lowercase hex sha"
cp "$work/pin" "$fx/wafer-run-pin.txt"

expect_check "everything restored" ok

# --- a spelling the rewrite cannot reach is refused, not skipped -------------
cat >> "$fx/blocks/beta/Cargo.toml" <<EOF

[dependencies.wafer-block]
git = "$url"
branch = "main"
EOF
before="$(cat "$fx/blocks/beta/Cargo.toml")"
if (cd "$fx" && scripts/set-wafer-run-rev.sh "$sha" >/dev/null 2>&1); then
  fail "set-wafer-run-rev.sh accepted a [dependencies.x] table it cannot rewrite"
else
  echo "ok: refuses a dependency table it cannot rewrite"
fi
[ "$(cat "$fx/blocks/beta/Cargo.toml")" = "$before" ] || fail "a refused rewrite still modified a manifest"

if [ "$failures" -ne 0 ]; then
  echo "set-wafer-run-rev.test.sh: $failures failure(s)" >&2
  exit 1
fi
echo "set-wafer-run-rev.test.sh OK"
