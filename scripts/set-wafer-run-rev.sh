#!/usr/bin/env bash
# Pin every wafer-run git dependency in this repo to one full commit sha, or
# verify that the pin holds.
#
#   scripts/set-wafer-run-rev.sh [-v] <40-hex-sha>
#       Rewrites each wafer-run git dependency in every Cargo.toml git knows
#       about (tracked, or untracked and not ignored: cli/, block-utils/,
#       blocks/**) to
#           { git = "https://github.com/wafer-run/wafer-run", rev = "<sha>", ... }
#       keeping every other key of the dependency (features, default-features,
#       optional, package, ...), then writes <sha> to wafer-run-pin.txt.
#       It does not touch any Cargo.lock: refresh those afterwards (see
#       CONTRIBUTING.md, "Bumping wafer-run").
#
#   scripts/set-wafer-run-rev.sh [-v] --check
#       Fails unless all of these hold:
#         - wafer-run-pin.txt is a full 40-character lowercase hex sha;
#         - every wafer-run git dependency in every Cargo.toml is spelled
#           exactly { git = "https://github.com/wafer-run/wafer-run",
#           rev = "<pin>" } (no `.git`, no ssh URL, no short rev, no tag);
#         - no git dependency of any kind uses `branch =`;
#         - every tracked Cargo.lock resolves wafer-run only from
#           git+https://github.com/wafer-run/wafer-run?rev=<pin>#<pin>, and the
#           cli/, block-utils/ and blocks/<slug>/ locks resolve it at all;
#         - no tracked Cargo.lock holds more than one wafer-block, wafer-sdk or
#           wafer-block-macro package.
#
#   -v  also lists the git-ignored Cargo.toml files (build output under
#       target/, other checkouts under .claude/worktrees/, ...) that are out
#       of scope.
#
# Only single-line inline-table dependencies are rewritten. Every other byte
# of the file is kept: whole-line comments, a trailing comment after the
# dependency (even one containing braces), and CRLF line endings. The rewrite
# is verified by parsing the manifest before and after: the two must be equal
# once the wafer-run source keys (git, branch, rev, tag) are set aside, and
# every wafer-run dependency must then pass --check. Comments are not part of
# the parsed manifest, so that verification cannot see them; they are kept by
# construction (the rewrite only replaces the text between the dependency's
# own braces). A spelling the rewrite cannot reach, such as a
# [dependencies.wafer-sdk] table or a features array split over lines, is
# refused and nothing is written.
#
# Why one spelling: Cargo identifies a git source by its URL plus its
# reference, so `branch = "main"` and `rev = "<sha>"`, or a short and a full
# sha, are different packages. Every block depends on block-utils by path, so
# one manifest on a different spelling puts two copies of wafer-block into
# that block's graph, and cargo builds it without complaint.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
  echo "usage: scripts/set-wafer-run-rev.sh [-v] <40-hex-sha> | [-v] --check" >&2
  exit 2
}

verbose=0
target=""
for arg in "$@"; do
  case "$arg" in
    -h | --help) usage ;;
    -v | --verbose) verbose=1 ;;
    *)
      [ -z "$target" ] || usage
      target="$arg"
      ;;
  esac
done
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ] || [ -z "$target" ]; then
  usage
fi

python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)' 2>/dev/null || {
  echo "error: scripts/set-wafer-run-rev.sh needs python3 >= 3.11 (for tomllib); found: $(python3 --version 2>&1 || echo none)" >&2
  exit 2
}

exec python3 - "$root" "$target" "$verbose" <<'PY'
import copy
import os
import re
import subprocess
import sys
import tomllib

root, arg, verbose = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
PIN_FILE = os.path.join(root, "wafer-run-pin.txt")
URL = "https://github.com/wafer-run/wafer-run"
SHA_RE = re.compile(r"[0-9a-f]{40}")
# Locks that must resolve wafer-run at all: the CLI, block-utils and every
# top-level block crate.
REQUIRED_LOCK_RE = re.compile(r"(cli|block-utils|blocks/[^/]+)/Cargo\.lock")
SINGLE_CRATES = ("wafer-block", "wafer-sdk", "wafer-block-macro")


def is_sha(value):
    return SHA_RE.fullmatch(value) is not None


def is_wafer_run(git_url):
    u = git_url.strip().lower()
    u = re.sub(r"^(git\+)?(https?|ssh|git)://", "", u)
    u = re.sub(r"^[^@/]+@", "", u)  # user@ of an ssh URL
    u = re.sub(r"^github\.com:", "github.com/", u)  # scp-style git@github.com:owner/repo
    u = u.rstrip("/")
    if u.endswith(".git"):
        u = u[: -len(".git")]
    return u == "github.com/wafer-run/wafer-run"


def git_ls(*args):
    out = subprocess.run(
        ["git", "-C", root, "ls-files", "-z", *args],
        check=True,
        capture_output=True,
    ).stdout.decode()
    return sorted({p for p in out.split("\0") if p})


def manifests():
    # Every Cargo.toml git knows about: tracked, or new and not ignored (a
    # freshly scaffolded block). Ignored ones are build output or other
    # checkouts of this repo and are never ours to rewrite; -v lists them.
    paths = git_ls("--cached", "--others", "--exclude-standard", "--", "*Cargo.toml")
    return [os.path.join(root, p) for p in paths if os.path.isfile(os.path.join(root, p))]


def report_ignored():
    ignored = git_ls("--others", "--ignored", "--exclude-standard", "--", "*Cargo.toml")
    print(f"Out of scope (git-ignored): {len(ignored)} Cargo.toml file(s)")
    for p in ignored:
        print(f"  skipped {p}")


def rel(path):
    return os.path.relpath(path, root)


def read(path):
    # newline="" keeps CRLF line endings intact through the round trip.
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


def git_specs(node, trail=()):
    """Yield (trail, dict) for every dependency spec that carries a `git` key."""
    if isinstance(node, dict):
        if isinstance(node.get("git"), str):
            yield trail, node
        for key, value in node.items():
            yield from git_specs(value, trail + (str(key),))
    elif isinstance(node, list):
        for i, value in enumerate(node):
            yield from git_specs(value, trail + (str(i),))


def manifest_errors(path, text, pin):
    try:
        tree = tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        return [f"{rel(path)}: not valid TOML ({e})"]
    errors = []
    for trail, spec in git_specs(tree):
        where = f"{rel(path)}: {'.'.join(trail)}"
        if "branch" in spec:
            errors.append(f"{where} uses branch = {spec['branch']!r}; pin a full rev instead")
        if not is_wafer_run(spec["git"]):
            continue
        if spec["git"] != URL:
            errors.append(f"{where} spells the wafer-run URL {spec['git']!r}; use {URL!r}")
        if "tag" in spec:
            errors.append(f"{where} pins wafer-run by tag {spec['tag']!r}; use rev = {pin!r} alone")
        if spec.get("rev") != pin:
            errors.append(f"{where} pins wafer-run at rev {spec.get('rev')!r}; wafer-run-pin.txt says {pin!r}")
    return errors


# --- rewriting --------------------------------------------------------------

DEP_HEAD_RE = re.compile(r"(?P<indent>[ \t]*)(?P<key>[A-Za-z0-9_.\-\"']+)(?P<eq>[ \t]*=[ \t]*)\{")
TAIL_RE = re.compile(r"[ \t]*(#.*)?")


def scan(text, start):
    """Walk text from start, aware of quotes and nesting. Return (items, end):
    the top-level comma-separated items and the index of the closing brace
    that ends the inline table, or (None, None) if it does not close."""
    items, buf, depth, quote, i = [], [], 0, None, start
    while i < len(text):
        c = text[i]
        if quote:
            buf.append(c)
            if quote == '"' and c == "\\" and i + 1 < len(text):
                buf.append(text[i + 1])
                i += 1
            elif c == quote:
                quote = None
        elif c in "\"'":
            quote = c
            buf.append(c)
        elif c in "[{":
            depth += 1
            buf.append(c)
        elif c in "]}" and depth > 0:
            depth -= 1
            buf.append(c)
        elif c == "}":
            last = "".join(buf).strip()
            if last:
                items.append(last)
            return items, i
        elif c == "," and depth == 0:
            items.append("".join(buf).strip())
            buf = []
        else:
            buf.append(c)
        i += 1
    return None, None


def item_key(item):
    return item.split("=", 1)[0].strip().strip("\"'")


def rewrite_line(line, sha):
    eol = "\r" if line.endswith("\r") else ""
    content = line[: len(line) - len(eol)]
    m = DEP_HEAD_RE.match(content)
    if not m:
        return line
    items, close = scan(content, m.end())
    if items is None or not TAIL_RE.fullmatch(content[close + 1 :]):
        return line
    git_item = next((it for it in items if item_key(it) == "git"), None)
    if git_item is None:
        return line
    try:
        git_url = tomllib.loads(git_item)["git"]
    except (tomllib.TOMLDecodeError, KeyError):
        return line
    if not is_wafer_run(git_url):
        return line
    kept = [it for it in items if item_key(it) not in ("git", "branch", "rev", "tag")]
    new_items = [f'git = "{URL}"', f'rev = "{sha}"'] + kept
    return f"{content[: m.end() - 1]}{{ {', '.join(new_items)} }}{content[close + 1 :]}{eol}"


def without_wafer_refs(tree):
    """The manifest with every wafer-run spec's source keys removed."""
    tree = copy.deepcopy(tree)
    for _, spec in git_specs(tree):
        if is_wafer_run(spec["git"]):
            for key in ("git", "branch", "rev", "tag"):
                spec.pop(key, None)
    return tree


def set_rev(sha):
    if not is_sha(sha):
        sys.exit(f"error: {sha!r} is not a full 40-character lowercase hex sha")
    changed, errors, plans = [], [], []
    paths = manifests()
    for path in paths:
        text = read(path)
        new_text = "\n".join(rewrite_line(line, sha) for line in text.split("\n"))
        try:
            before = tomllib.loads(text)
            after = tomllib.loads(new_text)
        except tomllib.TOMLDecodeError as e:
            errors.append(f"{rel(path)}: not valid TOML before or after the rewrite ({e})")
            continue
        if without_wafer_refs(before) != without_wafer_refs(after):
            errors.append(f"{rel(path)}: rewrite changed more than the wafer-run source keys")
            continue
        # A spelling the line rewrite cannot reach (e.g. a [dependencies.x]
        # table) is left alone and reported here, so nothing is missed silently.
        errors.extend(manifest_errors(path, new_text, sha))
        if new_text != text:
            plans.append((path, new_text))
    if verbose:
        report_ignored()
    if errors:
        print("\n".join(f"error: {e}" for e in errors), file=sys.stderr)
        sys.exit("error: nothing was written; fix the manifests above and rerun")
    for path, new_text in plans:
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(new_text)
        changed.append(rel(path))
    with open(PIN_FILE, "w", encoding="utf-8") as f:
        f.write(sha + "\n")
    print(f"Pinned wafer-run at {sha}: rewrote {len(changed)} of {len(paths)} manifest(s), wrote wafer-run-pin.txt.")
    print("Next: refresh every Cargo.lock that resolves wafer-run (see CONTRIBUTING.md).")


# --- checking ---------------------------------------------------------------

LOCK_SOURCE_RE = re.compile(r'^source = "(git\+[^"]*github\.com/wafer-run/wafer-run[^"]*)"\r?$', re.M)


def lock_errors(relpath, pin):
    text = read(os.path.join(root, relpath))
    errors = []
    expected = f"git+{URL}?rev={pin}#{pin}"
    sources = sorted(set(LOCK_SOURCE_RE.findall(text)))
    if not sources and REQUIRED_LOCK_RE.fullmatch(relpath):
        errors.append(f"{relpath} does not resolve wafer-run at all")
    for source in sources:
        if source != expected:
            errors.append(f"{relpath} resolves wafer-run from {source}; expected {expected}")
    for crate in SINGLE_CRATES:
        n = len(re.findall(rf'^name = "{re.escape(crate)}"\r?$', text, re.M))
        if n > 1:
            errors.append(f"{relpath} holds {n} copies of {crate}; a block graph must link exactly one")
    return errors


def check():
    try:
        pin = read(PIN_FILE).strip()
    except FileNotFoundError:
        sys.exit("error: wafer-run-pin.txt is missing")
    errors = []
    if not is_sha(pin):
        errors.append(f"wafer-run-pin.txt holds {pin!r}, not a full 40-character lowercase hex sha")
    paths = manifests()
    for path in paths:
        errors.extend(manifest_errors(path, read(path), pin))
    locks = [p for p in git_ls("--cached", "--", "*Cargo.lock") if os.path.isfile(os.path.join(root, p))]
    for lock in locks:
        errors.extend(lock_errors(lock, pin))
    if verbose:
        report_ignored()
    if errors:
        for e in errors:
            print(f"::error::{e}")
        print(f"{len(errors)} wafer-run pin problem(s) across {len(paths)} manifest(s) and {len(locks)} lock(s).")
        print("Bump procedure (CONTRIBUTING.md, 'Bumping wafer-run'):")
        print("  scripts/set-wafer-run-rev.sh <full sha>, refresh the cli/ and block-utils/ locks,")
        print("  then rebuild every block's Cargo.lock + target/block.wasm with scripts/build-block-wasm.sh")
        print("  (or the 'Refresh block WASM' workflow) and commit them.")
        sys.exit(1)
    print(f"wafer-run pin {pin}: {len(paths)} manifest(s) and {len(locks)} tracked lock(s) agree.")


if arg == "--check":
    check()
elif arg.startswith("-"):
    sys.exit(f"error: unknown option {arg!r}")
else:
    set_rev(arg)
PY
