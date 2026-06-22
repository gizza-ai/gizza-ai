#!/usr/bin/env python3
"""Pick the next backlog tool for /create-next-tool — smarter than a plain
first-un-built scan.

Layered filters (in order), each LOGGED to stderr so the loop is auditable:

  1. built        — `blocks/<slug>/` committed in git HEAD (a half-built failure
                    never counts, so a crashed run is retried, not skipped forever).
  2. skip-list    — `docs/tool-skiplist.txt`: hand-curated confirmed duplicates of
                    an existing tool, or rows that genuinely can't be built. Auto
                    token-overlap dup-detection is too noisy (it flags
                    age-calculator as a dup of calculator), so dups are curated, not
                    guessed. The build agent appends a line here when it spots a
                    semantic near-dup mid-run.
  3. out-of-model — needs an ML model / pyodide that gizza's pure-Rust+ffmpeg model
                    can't run (whisper, transformers-js, onnx, diffusion, …).
                    DEFERRED by default (pass --include-model to build anyway, e.g.
                    as a gpu chat-only block); never silently dropped.

Prints ONE line for the chosen tool:  <slug>\t<name>\t<description>\t<type_hint>
(type_hint ∈ pure|ffmpeg|network|model — a starting guess; the build step still
classifies). Or a sentinel: BACKLOG_COMPLETE / NO_BUILDABLE_REMAINING.

Usage:
  scripts/pick-next-tool.py                 # next buildable in-model tool
  scripts/pick-next-tool.py --include-model # don't defer model tools
  scripts/pick-next-tool.py --list 20       # preview the next N picks (no build)
  scripts/pick-next-tool.py --stats         # backlog breakdown
"""
import argparse
import csv
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CSV = os.path.join(ROOT, "tools-to-build.csv")
SKIPLIST = os.path.join(ROOT, "docs", "tool-skiplist.txt")

# feasibility/description signals. A row is IN-MODEL when its feasibility names a
# pure-Rust / ffmpeg path; that wins even if the description says "summarize",
# because the CSV's `loader_feasibility` is the authored build-path call.
IN_MODEL = re.compile(
    r"plain rust|pure[- ]?rust|rust\s*[→-]+\s*wasm|rust crate|`?\w+`?\s+crate|"
    r"ffmpeg|webcodecs|image crate|string assembly|numeric|calendar math|"
    r"regex|parser|wasm sandbox",
    re.I,
)
MODEL = re.compile(
    r"transformers[- ]?js|whisper|webgpu|onnx|diffusion|stable[- ]?diffusion|"
    r"\bllm\b|language model|\bpyodide\b|tesseract|ocr|embedding model|"
    r"neural|\bmodel\b",
    re.I,
)
FFMPEG = re.compile(
    r"ffmpeg|webcodecs|\bvideo\b|\baudio\b|transcod|re-?encode|waveform|\bgif\b",
    re.I,
)
NETWORK = re.compile(r"\bfetch(es|ing)?\b|downloads?|http request|scrape|crawl", re.I)


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.strip().lower()).strip("-")


def built_slugs():
    out = subprocess.run(
        ["git", "ls-tree", "-d", "--name-only", "HEAD", "blocks/"],
        capture_output=True, text=True, cwd=ROOT,
    ).stdout.split()
    return {b.split("/", 1)[1] for b in out if "/" in b}


def skip_slugs():
    skips = {}
    if os.path.exists(SKIPLIST):
        for line in open(SKIPLIST):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = re.split(r"\s+#\s*", line, maxsplit=1)
            skips[slug(parts[0])] = parts[1] if len(parts) > 1 else "(no reason given)"
    return skips


def type_hint(row):
    text = f"{row.get('loader_feasibility','')} {row.get('description','')}"
    if MODEL.search(text) and not IN_MODEL.search(text):
        return "model"
    if FFMPEG.search(text):
        return "ffmpeg"
    if NETWORK.search(text):
        return "network"
    return "pure"


def is_out_of_model(row):
    text = f"{row.get('loader_feasibility','')} {row.get('description','')}"
    return bool(MODEL.search(text)) and not IN_MODEL.search(text)


def candidates(include_model):
    built = built_slugs()
    skips = skip_slugs()
    for row in csv.DictReader(open(CSV, newline="")):
        s = slug(row["name"])
        if s in built:
            continue
        if s in skips:
            print(f"skip[dup/curated]  {s}  — {skips[s]}", file=sys.stderr)
            continue
        if not include_model and is_out_of_model(row):
            print(f"skip[out-of-model] {s}  — needs ML model/pyodide; not gizza pure+ffmpeg",
                  file=sys.stderr)
            continue
        yield s, row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--include-model", action="store_true",
                    help="don't defer model/pyodide tools")
    ap.add_argument("--list", type=int, metavar="N", help="preview next N picks")
    ap.add_argument("--stats", action="store_true", help="backlog breakdown")
    args = ap.parse_args()

    if args.stats:
        built = built_slugs()
        skips = skip_slugs()
        rows = list(csv.DictReader(open(CSV, newline="")))
        unbuilt = [r for r in rows if slug(r["name"]) not in built]
        oom = sum(1 for r in unbuilt if is_out_of_model(r))
        dup = sum(1 for r in unbuilt if slug(r["name"]) in skips)
        from collections import Counter
        th = Counter(type_hint(r) for r in unbuilt)
        print(f"total backlog      : {len(rows)}")
        print(f"built (git HEAD)   : {len(built)}")
        print(f"un-built           : {len(unbuilt)}")
        print(f"  curated-skipped  : {dup}")
        print(f"  out-of-model     : {oom}")
        print(f"  buildable now    : {len(unbuilt) - oom - dup}")
        print(f"type hints (unbuilt): {dict(th)}")
        return

    if args.list:
        n = 0
        for s, row in candidates(args.include_model):
            print(f"{s}\t{row['name']}\t{row['description'][:60]}\t{type_hint(row)}")
            n += 1
            if n >= args.list:
                break
        if n == 0:
            print("NO_BUILDABLE_REMAINING")
        return

    for s, row in candidates(args.include_model):
        print(f"{s}\t{row['name']}\t{row['description']}\t{type_hint(row)}")
        return
    # nothing yielded
    built = built_slugs()
    total_unbuilt = sum(1 for r in csv.DictReader(open(CSV, newline="")) if slug(r["name"]) not in built)
    print("BACKLOG_COMPLETE" if total_unbuilt == 0 else "NO_BUILDABLE_REMAINING")


if __name__ == "__main__":
    main()
