#!/usr/bin/env bash
# Self-test for scripts/set-wafer-run-rev.sh. Builds a throwaway repo with every
# wafer-run dependency spelling the script must handle (branch = "main", the
# `.git` URL, an ssh URL, a short rev, a tag, extra keys, a trailing comment
# with braces, CRLF line endings, dev and target-specific tables), pins it, and
# checks that the rewrite keeps every other byte and that --check refuses each
# way the pin can break: a branch, a tag, a short or mis-spelled rev, a bad pin
# file, a lock whose source differs from ?rev=<pin>#<pin> even at the pinned
# sha, and a lock holding two copies of wafer-block. Portable to bash 3.2.
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

# expect_check <description> <ok|fail> [text the output must contain] [extra flag]
expect_check() {
  local what="$1" want="$2" pattern="${3:-}" flag="${4:-}" out status=0
  out="$(cd "$fx" && scripts/set-wafer-run-rev.sh ${flag:+"$flag"} --check 2>&1)" || status=$?
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
  "$fx/blocks/gamma" "$fx/blocks/alpha/target/sub" "$fx/.claude/worktrees/other/blocks/x"
cp "$root/scripts/set-wafer-run-rev.sh" "$fx/scripts/"
echo "$old" > "$fx/wafer-run-pin.txt"
# The same ignore rules the repo uses for build output and other checkouts.
printf '**/target\n.claude/worktrees/\nCargo.lock\n' > "$fx/.gitignore"

cat > "$fx/cli/Cargo.toml" <<EOF
[package]
name = "cli"
version = "0.1.0"

[dependencies]
# whole-line comment: wafer-sdk = { git = "$url", branch = "main" }
wafer-run = { git = "$url", branch = "main", default-features = false, features = ["wasm", "x"] } # runtime
wafer-core = { git = "$url", rev = "914a0aad" } # see {notes}, default-features = false }
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
wafer-block = { git = "ssh://git@github.com/wafer-run/wafer-run.git", branch = "main" }
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

crlf() { sed 's/$/\r/'; }
crlf > "$fx/blocks/gamma/Cargo.toml" <<EOF
[package]
name = "gamma"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url.git", branch = "main" } # crlf
EOF

# Not ours to touch: build output and other checkouts (both git-ignored).
for d in "$fx/blocks/alpha/target/sub" "$fx/.claude/worktrees/other/blocks/x"; do
  printf '[package]\nname = "x"\nversion = "0.1.0"\n[dependencies]\nwafer-sdk = { git = "%s", branch = "main" }\n' "$url" > "$d/Cargo.toml"
done

git -C "$fx" init -q
git -C "$fx" add -A

# --- environment ------------------------------------------------------------
mkdir -p "$work/oldpy"
printf '#!/bin/sh\necho "Python 3.9.6"\nexit 1\n' > "$work/oldpy/python3"
chmod +x "$work/oldpy/python3"
out="$(cd "$fx" && PATH="$work/oldpy:$PATH" scripts/set-wafer-run-rev.sh --check 2>&1)" && status=0 || status=$?
if [ "$status" -ne 0 ] && grep -qF "needs python3 >= 3.11 (for tomllib); found: Python 3.9.6" <<<"$out"; then
  echo "ok: an old python3 gets a clear error"
else
  fail "an old python3 did not get the tomllib error (exit $status): $out"
fi

# --- argument validation ----------------------------------------------------
upper="$(printf '%s' "$sha" | tr 'a-f' 'A-F')"
nl="$sha
"
for bad in 914a0aad "$upper" "${sha}0" "g${sha#?}" "$nl"; do
  out="$(cd "$fx" && scripts/set-wafer-run-rev.sh "$bad" 2>&1)" && status=0 || status=$?
  if [ "$status" -ne 0 ] && grep -qF "is not a full 40-character lowercase hex sha" <<<"$out"; then
    echo "ok: refuses sha '$bad'"
  else
    fail "set-wafer-run-rev.sh did not refuse the sha '$bad' as malformed (exit $status): $out"
  fi
done
if (cd "$fx" && scripts/set-wafer-run-rev.sh "" >/dev/null 2>&1); then
  fail "set-wafer-run-rev.sh accepted an empty sha"
else
  echo "ok: refuses an empty sha"
fi
git -C "$fx" diff --quiet || fail "a refused sha still modified files"

expect_check "unpinned corpus is refused" fail 'uses branch = '"'main'"
expect_check "-v lists ignored build output" fail "skipped blocks/alpha/target/sub/Cargo.toml" -v
expect_check "-v lists ignored checkouts" fail "skipped .claude/worktrees/other/blocks/x/Cargo.toml" -v

# --- rewrite ----------------------------------------------------------------
(cd "$fx" && scripts/set-wafer-run-rev.sh "$sha" >/dev/null) || fail "set-wafer-run-rev.sh $sha failed"

[ "$(cat "$fx/wafer-run-pin.txt")" = "$sha" ] || fail "wafer-run-pin.txt was not written"

expect_file() { # <path> <expected text>
  if [ "$(cat "$fx/$1")" = "$2" ]; then
    echo "ok: $1 rewritten as expected"
  else
    fail "$1 rewrite differs from the expected text"
    diff <(printf '%s\n' "$2") "$fx/$1" >&2 || true
  fi
}

expect_file cli/Cargo.toml "$(cat <<EOF
[package]
name = "cli"
version = "0.1.0"

[dependencies]
# whole-line comment: wafer-sdk = { git = "$url", branch = "main" }
wafer-run = { git = "$url", rev = "$sha", default-features = false, features = ["wasm", "x"] } # runtime
wafer-core = { git = "$url", rev = "$sha" } # see {notes}, default-features = false }
"wafer-block-network" = { git = "$url", rev = "$sha" }
other-git = { git = "https://example.com/other/repo", rev = "0123456789abcdef0123456789abcdef01234567" }
serde = { version = "1", features = ["derive"] }
EOF
)"

expect_file block-utils/Cargo.toml "$(cat <<EOF
[package]
name = "block-utils"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url", rev = "$sha" }
wafer-block = { git = "$url", rev = "$sha" }
EOF
)"

expect_file blocks/alpha/Cargo.toml "$(cat <<EOF
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

want_gamma="$(crlf <<EOF
[package]
name = "gamma"
version = "0.1.0"

[dependencies]
wafer-sdk = { git = "$url", rev = "$sha" } # crlf
EOF
)"
if [ "$(cat "$fx/blocks/gamma/Cargo.toml")" = "$want_gamma" ]; then
  echo "ok: CRLF line endings survive the rewrite"
else
  fail "blocks/gamma/Cargo.toml lost its CRLF line endings or was rewritten wrongly"
  od -c "$fx/blocks/gamma/Cargo.toml" | tail -5 >&2
fi

git -C "$fx" diff --quiet -- blocks/alpha/core/Cargo.toml \
  || fail "the rewrite touched a manifest with no wafer-run dependency"
for ignored in blocks/alpha/target/sub/Cargo.toml .claude/worktrees/other/blocks/x/Cargo.toml; do
  grep -q 'branch = "main"' "$fx/$ignored" || fail "the rewrite touched the git-ignored $ignored"
done

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

# Each wrong source below is at the PINNED sha, so only an exact-source
# comparison catches it; a check of the sha after '#' would pass them.
write_locks "git+$url?branch=main#$sha"
expect_check "lock on a branch, at the pinned sha" fail "blocks/beta/Cargo.lock resolves wafer-run from git+$url?branch=main#$sha"

write_locks "git+$url.git?rev=$sha#$sha"
expect_check "lock from the .git URL, at the pinned sha" fail "blocks/beta/Cargo.lock resolves wafer-run from git+$url.git?rev=$sha#$sha"

write_locks "git+$url?rev=${sha:0:8}#$sha"
expect_check "lock from a short rev, at the pinned sha" fail "blocks/beta/Cargo.lock resolves wafer-run from git+$url?rev=${sha:0:8}#$sha"

write_locks "git+$url?rev=$old#$old"
expect_check "lock at another sha" fail "blocks/beta/Cargo.lock resolves wafer-run from git+$url?rev=$old#$old"

write_locks "$good"
lock_entry wafer-block "$good" >> "$fx/blocks/beta/Cargo.lock"
git -C "$fx" add -f blocks/beta/Cargo.lock
expect_check "lock with two wafer-block packages" fail "blocks/beta/Cargo.lock holds 2 copies of wafer-block"

write_locks "$good"
{ lock_entry wafer-sdk "$good"; lock_entry wafer-sdk "$good"; } > "$fx/block-utils/Cargo.lock"
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

mutate blocks/beta/Cargo.toml "s|rev = \"$sha\"|rev = \"$sha\", tag = \"v1.0.0\"|"
expect_check "rev and tag together" fail "blocks/beta/Cargo.toml: dependencies.wafer-sdk pins wafer-run by tag 'v1.0.0'"
restore

mutate cli/Cargo.toml "s|rev = \"$sha\" }$|rev = \"${sha:0:8}\" }|"
expect_check "short rev" fail "pins wafer-run at rev '${sha:0:8}'"
restore

mutate blocks/beta/Cargo.toml "s|$url\"|$url.git\"|"
expect_check "the .git URL spelling" fail "spells the wafer-run URL '$url.git'"
restore

mutate blocks/beta/Cargo.toml "s|$url\"|ssh://git@github.com/wafer-run/wafer-run\"|"
expect_check "an ssh URL" fail "spells the wafer-run URL 'ssh://git@github.com/wafer-run/wafer-run'"
restore

mutate cli/Cargo.toml 's|other/repo", rev = "[0-9a-f]*"|other/repo", branch = "dev"|'
expect_check "a non-wafer-run git dependency on a branch" fail "dependencies.other-git uses branch = 'dev'"
restore

cp "$fx/wafer-run-pin.txt" "$work/pin"
echo "${sha:0:12}" > "$fx/wafer-run-pin.txt"
expect_check "short sha in wafer-run-pin.txt" fail "not a full 40-character lowercase hex sha"
cp "$work/pin" "$fx/wafer-run-pin.txt"

expect_check "everything restored" ok

# --- rewrites that must be refused, with nothing written ---------------------
expect_refused() { # <description> <file> <message>
  local before out status=0
  before="$(cat "$fx/$2")"
  out="$(cd "$fx" && scripts/set-wafer-run-rev.sh "$sha" 2>&1)" || status=$?
  if [ "$status" -eq 0 ]; then
    fail "$1: the rewrite was accepted"
  elif ! grep -qF -- "$3" <<<"$out"; then
    fail "$1: output lacks '$3'"
    echo "$out" >&2
  elif [ "$(cat "$fx/$2")" != "$before" ]; then
    fail "$1: a refused rewrite still modified $2"
  else
    echo "ok: refuses $1"
  fi
}

# A line inside a multi-line string looks like a dependency to the line
# rewriter; only the parsed-equality check notices the string would change.
mkdir -p "$fx/blocks/delta"
cat > "$fx/blocks/delta/Cargo.toml" <<EOF
[package]
name = "delta"
version = "0.1.0"
description = """
wafer-sdk = { git = "$url", branch = "main" }
"""

[dependencies]
wafer-sdk = { git = "$url", rev = "$sha" }
EOF
expect_refused "a rewrite that would change a string value" blocks/delta/Cargo.toml \
  "blocks/delta/Cargo.toml: rewrite changed more than the wafer-run source keys"
rm -rf "$fx/blocks/delta"

cp "$fx/blocks/beta/Cargo.toml" "$work/beta"
cat >> "$fx/blocks/beta/Cargo.toml" <<EOF

[dependencies.wafer-block]
git = "$url"
branch = "main"
EOF
expect_refused "a [dependencies.x] table it cannot rewrite" blocks/beta/Cargo.toml \
  "blocks/beta/Cargo.toml: dependencies.wafer-block uses branch = 'main'"
cp "$work/beta" "$fx/blocks/beta/Cargo.toml"

expect_check "refusals left the repo pinned" ok

if [ "$failures" -ne 0 ]; then
  echo "set-wafer-run-rev.test.sh: $failures failure(s)" >&2
  exit 1
fi
echo "set-wafer-run-rev.test.sh OK"
