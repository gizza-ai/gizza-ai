#!/usr/bin/env bash
# Self-test for scripts/check-tool-hygiene.py — asserts the gate FAILS on a block
# with a drifted manifest + plain-markdown FAQ, and PASSES once both are fixed.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
slug="zzhygiene-scratch"
dir="$root/blocks/$slug"
cleanup() { rm -rf "$dir"; }
trap cleanup EXIT
rm -rf "$dir"
mkdir -p "$dir/src" "$dir/page"

# A real (non-comment) enum param.
cat > "$dir/src/lib.rs" <<'EOF'
fn descriptor() {
    let _ = Param::enumv("mode", ["encode", "decode"]).default("encode");
}
EOF

# BAD manifest: `mode` present but with NO enum (the drift that renders a text box),
# plus a leftover scaffold TODO.
cat > "$dir/manifest.json" <<'EOF'
{ "summary": "TODO: one-line summary.", "tool": { "parameters": { "properties": {
  "mode": { "type": "string", "default": "encode" }
} } } }
EOF

# BAD content: FAQ as plain markdown, no <details>.
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

**Is it free?** Yes.
EOF

if python3 "$root/scripts/check-tool-hygiene.py" "$slug" >/dev/null 2>&1; then
  echo "FAIL: gate passed a drifted+plain-FAQ block (should have failed)" >&2
  exit 1
fi
out="$(python3 "$root/scripts/check-tool-hygiene.py" "$slug" 2>&1 || true)"
grep -q 'TEXT BOX' <<<"$out" || { echo "FAIL: missing enum-drift violation" >&2; exit 1; }
grep -q 'FAQ section written as plain markdown' <<<"$out" || { echo "FAIL: missing FAQ violation" >&2; exit 1; }
grep -q "scaffold 'TODO' placeholder" <<<"$out" || { echo "FAIL: missing TODO violation" >&2; exit 1; }

# FIX all and expect a clean pass.
cat > "$dir/manifest.json" <<'EOF'
{ "summary": "A real one-line summary.", "tool": { "parameters": { "properties": {
  "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode" }
} } } }
EOF
cat > "$dir/page/content.md" <<'EOF'
## About
Prose.

## FAQ

<details>
<summary>Is it free?</summary>
<p>Yes.</p>
</details>
EOF

python3 "$root/scripts/check-tool-hygiene.py" "$slug" >/dev/null 2>&1 || {
  echo "FAIL: gate rejected a compliant block" >&2; exit 1; }

echo "check-tool-hygiene.test.sh OK"
