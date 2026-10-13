#!/usr/bin/env bash
# Pin every wafer-run git dependency in this repo to one full commit sha, or
# verify that the pin holds.
#
#   scripts/set-wafer-run-rev.sh <40-hex-sha>
#       Rewrites each wafer-run git dependency in every Cargo.toml (cli/,
#       block-utils/, blocks/**, and anything else outside target/, pkg/,
#       node_modules/ and dot-directories) to
#           { git = "https://github.com/wafer-run/wafer-run", rev = "<sha>", ... }
#       keeping every other key of the dependency (features, default-features,
#       optional, package, ...), then writes <sha> to wafer-run-pin.txt.
#       It does not touch any Cargo.lock: refresh those afterwards (see
#       CONTRIBUTING.md, "Bumping wafer-run").
#
#   scripts/set-wafer-run-rev.sh --check
#       Fails unless all of these hold:
#         - wafer-run-pin.txt is a full 40-character lowercase hex sha;
#         - every wafer-run git dependency in every Cargo.toml is spelled
#           exactly { git = "https://github.com/wafer-run/wafer-run",
#           rev = "<pin>" } (no `.git`, no short rev, no tag);
#         - no git dependency of any kind uses `branch =`;
#         - every tracked Cargo.lock resolves wafer-run only from
#           git+https://github.com/wafer-run/wafer-run?rev=<pin>#<pin>, and the
#           cli/, block-utils/ and blocks/<slug>/ locks resolve it at all;
#         - no tracked Cargo.lock holds more than one wafer-block, wafer-sdk or
#           wafer-block-macro package.
#
# Why one spelling: Cargo identifies a git source by its URL plus its
# reference, so `branch = "main"` and `rev = "<sha>"`, or a short and a full
# sha, are different packages. Every block depends on block-utils by path, so
# one manifest on a different spelling puts two copies of wafer-block into
# that block's graph.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
  echo "usage: scripts/set-wafer-run-rev.sh <40-hex-sha> | --check" >&2
  exit 2
}

[ "$#" -eq 1 ] || usage
case "$1" in
  -h | --help) usage ;;
esac

exec python3 - "$root" "$1" <<'PY'
import copy
import os
import re
import subprocess
import sys
import tomllib

root, arg = sys.argv[1], sys.argv[2]
PIN_FILE = os.path.join(root, "wafer-run-pin.txt")
URL = "https://github.com/wafer-run/wafer-run"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
# Build output, dependencies, and dot-directories (.git, and the .claude/ and
# .worktrees/ trees that hold other checkouts of this repo).
PRUNE = {"target", "pkg", "node_modules"}
# Locks that must resolve wafer-run at all: the CLI, block-utils and every
# top-level block crate.
REQUIRED_LOCK_RE = re.compile(r"^(cli|block-utils|blocks/[^/]+)/Cargo\.lock$")
SINGLE_CRATES = ("wafer-block", "wafer-sdk", "wafer-block-macro")


def is_wafer_run(git_url):
    u = git_url.strip().lower()
    u = re.sub(r"^(git\+)?(https?|ssh|git)://", "", u)
    u = re.sub(r"^git@github\.com:", "github.com/", u)
    u = u.rstrip("/")
    if u.endswith(".git"):
        u = u[: -len(".git")]
    return u == "github.com/wafer-run/wafer-run"


def manifests():
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in PRUNE and not d.startswith("."))
        if "Cargo.toml" in filenames:
            found.append(os.path.join(dirpath, "Cargo.toml"))
    return found


def rel(path):
    return os.path.relpath(path, root)


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
            errors.append(f"{where} pins wafer-run by tag {spec['tag']!r}; use rev = {pin!r}")
        if spec.get("rev") != pin:
            errors.append(f"{where} pins wafer-run at rev {spec.get('rev')!r}; wafer-run-pin.txt says {pin!r}")
    return errors


# --- rewriting --------------------------------------------------------------

INLINE_DEP_RE = re.compile(
    r"^(?P<indent>[ \t]*)(?P<key>[A-Za-z0-9_.\-\"']+)(?P<eq>[ \t]*=[ \t]*)"
    r"\{(?P<body>.*)\}(?P<tail>[ \t]*(#.*)?)$"
)


def split_items(body):
    """Split an inline table body on top-level commas (quotes and nesting aware)."""
    items, buf, depth, quote, i = [], [], 0, None, 0
    while i < len(body):
        c = body[i]
        if quote:
            buf.append(c)
            if quote == '"' and c == "\\" and i + 1 < len(body):
                buf.append(body[i + 1])
                i += 1
            elif c == quote:
                quote = None
        elif c in "\"'":
            quote = c
            buf.append(c)
        elif c in "[{":
            depth += 1
            buf.append(c)
        elif c in "]}":
            depth -= 1
            buf.append(c)
        elif c == "," and depth == 0:
            items.append("".join(buf).strip())
            buf = []
        else:
            buf.append(c)
        i += 1
    last = "".join(buf).strip()
    if last:
        items.append(last)
    return items


def item_key(item):
    return item.split("=", 1)[0].strip().strip("\"'")


def rewrite_line(line, sha):
    m = INLINE_DEP_RE.match(line)
    if not m:
        return line
    items = split_items(m.group("body"))
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
    return f"{m.group('indent')}{m.group('key')}{m.group('eq')}{{ {', '.join(new_items)} }}{m.group('tail')}"


def without_wafer_refs(tree):
    """The manifest with every wafer-run spec's source keys removed."""
    tree = copy.deepcopy(tree)
    for _, spec in git_specs(tree):
        if is_wafer_run(spec["git"]):
            for key in ("git", "branch", "rev", "tag"):
                spec.pop(key, None)
    return tree


def set_rev(sha):
    if not SHA_RE.match(sha):
        sys.exit(f"error: {sha!r} is not a full 40-character lowercase hex sha")
    changed, errors, plans = [], [], []
    for path in manifests():
        with open(path, encoding="utf-8") as f:
            text = f.read()
        lines = text.split("\n")
        new_text = "\n".join(rewrite_line(line, sha) for line in lines)
        try:
            before = tomllib.loads(text)
            after = tomllib.loads(new_text)
        except tomllib.TOMLDecodeError as e:
            errors.append(f"{rel(path)}: not valid TOML ({e})")
            continue
        if without_wafer_refs(before) != without_wafer_refs(after):
            errors.append(f"{rel(path)}: rewrite changed more than the wafer-run source keys")
            continue
        # A spelling the line rewrite cannot reach (e.g. a [dependencies.x]
        # table) is left alone and reported here, so nothing is missed silently.
        errors.extend(manifest_errors(path, new_text, sha))
        if new_text != text:
            plans.append((path, new_text))
    if errors:
        print("\n".join(f"error: {e}" for e in errors), file=sys.stderr)
        sys.exit("error: nothing was written; fix the manifests above and rerun")
    for path, new_text in plans:
        with open(path, "w", encoding="utf-8") as f:
            f.write(new_text)
        changed.append(rel(path))
    with open(PIN_FILE, "w", encoding="utf-8") as f:
        f.write(sha + "\n")
    print(f"Pinned wafer-run at {sha}: rewrote {len(changed)} manifest(s), wrote wafer-run-pin.txt.")
    print("Next: refresh every Cargo.lock that resolves wafer-run (see CONTRIBUTING.md).")


# --- checking ---------------------------------------------------------------

LOCK_SOURCE_RE = re.compile(r'^source = "(git\+[^"]*github\.com/wafer-run/wafer-run[^"]*)"$', re.M)


def tracked_locks():
    out = subprocess.run(
        ["git", "-C", root, "ls-files", "-z", "--", "*Cargo.lock"],
        check=True,
        capture_output=True,
    ).stdout.decode()
    return sorted(p for p in out.split("\0") if p)


def lock_errors(relpath, pin):
    with open(os.path.join(root, relpath), encoding="utf-8") as f:
        text = f.read()
    errors = []
    expected = f"git+{URL}?rev={pin}#{pin}"
    sources = sorted(set(LOCK_SOURCE_RE.findall(text)))
    if not sources and REQUIRED_LOCK_RE.match(relpath):
        errors.append(f"{relpath} does not resolve wafer-run at all")
    for source in sources:
        if source != expected:
            errors.append(f"{relpath} resolves wafer-run from {source}; expected {expected}")
    for crate in SINGLE_CRATES:
        n = len(re.findall(rf'^name = "{re.escape(crate)}"$', text, re.M))
        if n > 1:
            errors.append(f"{relpath} holds {n} copies of {crate}; a block graph must link exactly one")
    return errors


def check():
    try:
        with open(PIN_FILE, encoding="utf-8") as f:
            pin = f.read().strip()
    except FileNotFoundError:
        sys.exit("error: wafer-run-pin.txt is missing")
    errors = []
    if not SHA_RE.match(pin):
        errors.append(f"wafer-run-pin.txt holds {pin!r}, not a full 40-character lowercase hex sha")
    paths = manifests()
    for path in paths:
        with open(path, encoding="utf-8") as f:
            errors.extend(manifest_errors(path, f.read(), pin))
    locks = tracked_locks()
    for lock in locks:
        errors.extend(lock_errors(lock, pin))
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
