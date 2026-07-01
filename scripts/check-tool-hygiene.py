#!/usr/bin/env python3
"""Tool-hygiene gate — hard-fails when a block ships a defect the page renderer
can't fix on its own. Two checks, both traced to real regressions:

  1. enum→manifest drift. The page form (`tools/generator/src/control.rs`) reads
     `manifest.json` → `tool.parameters.properties`, NOT the live descriptor, and
     renders a `<select>` ONLY when the param carries an `enum` there. So a
     `Param::enumv(...)` in `src/lib.rs` whose `manifest.json` lost the `enum`
     silently renders a plain TEXT BOX instead of a dropdown. This gate requires
     every real `enumv` param to have a matching `enum` in the manifest (and, when
     the variant list is parseable, the SAME variants).

  2. FAQ formatting. `site/tool.css` styles FAQ as `<details>`/`<summary>`
     accordions (`.tool-content details ...`), but only if `page/content.md`
     actually uses that markup. A FAQ written as plain `## FAQ` markdown renders
     as bare headings. This gate requires any content.md with a FAQ section to use
     `<details>` accordions.

Prose usability standards (see `.claude/skills/improve-tool/SKILL.md`) were being
skipped silently because nothing failed the build. This turns the two mechanically
checkable ones into a gate.

Usage:
  scripts/check-tool-hygiene.py                 # check every blocks/<slug>/
  scripts/check-tool-hygiene.py <slug> [<slug>] # check only these tools

Exit code 0 = clean, 1 = violations found (prints each), 2 = usage error.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BLOCKS = ROOT / "blocks"

# `Param::enumv("name", ["a", "b", ...])` — name plus (optionally) the literal
# variant list. Non-greedy across the comment-stripped, whitespace-joined blob so
# multi-line calls match. The variant list may be absent from the capture if it's
# built from a const/helper rather than an inline array.
ENUMV_RE = re.compile(r'Param::enumv\(\s*"([^"]+)"\s*(?:,\s*(?:&?\[([^\]]*)\])?)?')
STR_LIT_RE = re.compile(r'"([^"]*)"')
FAQ_HEADING_RE = re.compile(r"^#{1,6}\s*(faq|frequently asked)", re.IGNORECASE | re.MULTILINE)


_RUST_ESCAPES = {"\\": "\\", '"': '"', "'": "'", "n": "\n", "t": "\t", "r": "\r", "0": "\0"}


def unescape_rust(s: str) -> str:
    """Resolve the common Rust string escapes so a source token like `\\x`
    (two backslashes in the .rs file) compares equal to the manifest's
    JSON-decoded `\\x` (one backslash). Without this, any enum variant containing
    a backslash or quote false-flags as STALE."""
    out: list[str] = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] in _RUST_ESCAPES:
            out.append(_RUST_ESCAPES[s[i + 1]])
            i += 2
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def strip_line_comments(src: str) -> str:
    """Drop whole-line `//`/`///` doc comments (the scaffold's descriptor doc
    comment contains a `Param::enumv("mode", ...)` EXAMPLE that must not count as a
    real param) and join the rest so multi-line calls match as one blob."""
    kept = [ln for ln in src.splitlines() if not ln.lstrip().startswith("//")]
    return " ".join(kept)


def descriptor_enums(src_lib: Path) -> dict[str, set[str] | None]:
    """name -> set of variants (or None when the variant list isn't an inline
    literal we can parse). Only real (non-comment) `Param::enumv` calls."""
    blob = strip_line_comments(src_lib.read_text(encoding="utf-8", errors="replace"))
    out: dict[str, set[str] | None] = {}
    for m in ENUMV_RE.finditer(blob):
        name = m.group(1)
        arr = m.group(2)
        variants = {unescape_rust(v) for v in STR_LIT_RE.findall(arr)} if arr is not None else None
        # If the same name appears twice, prefer a parseable variant set.
        if name not in out or (out[name] is None and variants is not None):
            out[name] = variants
    return out


def manifest_props(manifest: Path) -> dict | None:
    try:
        data = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return (
        data.get("tool", {})
        .get("parameters", {})
        .get("properties")
    )


def check_block(slug_dir: Path) -> list[str]:
    slug = slug_dir.name
    problems: list[str] = []

    src_lib = slug_dir / "src" / "lib.rs"
    manifest = slug_dir / "manifest.json"
    if src_lib.is_file() and manifest.is_file():
        enums = descriptor_enums(src_lib)
        if enums:
            props = manifest_props(manifest)
            if props is None:
                problems.append(
                    f"{slug}: has enum param(s) but manifest.json tool.parameters.properties is missing/unreadable"
                )
            else:
                for name, variants in enums.items():
                    prop = props.get(name)
                    man_enum = prop.get("enum") if isinstance(prop, dict) else None
                    if not isinstance(man_enum, list) or not man_enum:
                        problems.append(
                            f"{slug}: descriptor Param::enumv(\"{name}\") but manifest.json "
                            f"properties.{name} has no enum → manifest is out of sync with the descriptor "
                            f"(a page tool renders a TEXT BOX instead of a <select>). "
                            f"Sync manifest.json tool.parameters to the descriptor."
                        )
                    elif variants is not None and set(man_enum) != variants:
                        problems.append(
                            f"{slug}: enum param \"{name}\" is STALE — descriptor {sorted(variants)} "
                            f"vs manifest {sorted(map(str, man_enum))}. Re-sync manifest.json."
                        )

    content = slug_dir / "page" / "content.md"
    if content.is_file():
        text = content.read_text(encoding="utf-8", errors="replace")
        if FAQ_HEADING_RE.search(text) and "<details" not in text:
            problems.append(
                f"{slug}: page/content.md has a FAQ section written as plain markdown — "
                f"convert it to <details>/<summary>/<p> accordions (see blocks/age-calculator, "
                f"improve-tool usability standard #8) so site/tool.css styles it."
            )

    return problems


def main(argv: list[str]) -> int:
    if argv:
        dirs = []
        for slug in argv:
            d = BLOCKS / slug
            if not d.is_dir():
                print(f"error: blocks/{slug}/ not found", file=sys.stderr)
                return 2
            dirs.append(d)
    else:
        dirs = sorted(p for p in BLOCKS.iterdir() if p.is_dir())

    all_problems: list[str] = []
    for d in dirs:
        all_problems.extend(check_block(d))

    if all_problems:
        print(f"tool-hygiene: {len(all_problems)} violation(s) across {len(dirs)} tool(s):\n")
        for p in all_problems:
            print(f"  ✗ {p}")
        print("\nFAIL — fix the above before committing (see scripts/check-tool-hygiene.py header).")
        return 1

    print(f"tool-hygiene: OK — {len(dirs)} tool(s) clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
