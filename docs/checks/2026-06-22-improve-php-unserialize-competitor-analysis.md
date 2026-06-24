# php-unserialize — competitor analysis (2026-06-22)

Snapshot taken while building the `php-unserialize` tool (PHP `serialize()` string →
JSON). Companion to the existing `php-serialize` tool (JSON → PHP), of which this is
the inverse. All findings are paraphrased; no competitor copy/branding was reused.

## Competitors surveyed (top 5)

1. **Unserialize Guru** (unserializeguru.com) — unserialize PHP + JSON, beautify
   serialized data, side-by-side compare, claims 100% client-side processing.
2. **JSON Love** (jsonlove.com) — "PHP serialized data → JSON" one-way converter.
3. **Web Lite Solutions** (solutions.weblite.ca/php2json) — bidirectional PHP
   serialized ↔ JSON converter.
4. **FastMinify** (fastminify.com/en/php-serialize) — free PHP ↔ JSON serialize +
   unserialize.
5. **DUzun.Me playground** (duzun.me/playground/serialize) — PHP serialize/unserialize
   to JSON, advertises tolerance for "wrong encoding" / broken byte-length strings.

(Others noted: phphub.net, unserialize.com, devpicker.com — same shape.)

## Feature diff vs our tool

| Capability | Competitors | gizza php-unserialize | Verdict |
| --- | --- | --- | --- |
| Decode all scalar types (N/b/i/d/s) | yes | yes | parity |
| Decode arrays (list + assoc) | yes | yes — sequential-int keys → JSON array, otherwise JSON object | parity |
| Decode objects (`O:`) | most | yes — class name preserved under `__class` | parity / slight edge (class kept) |
| Byte-accurate string length (UTF-8) | mixed | yes — slices exactly `<len>` bytes, so embedded `;`/`"` are safe | parity / edge |
| Pretty-printed JSON output | yes (beautify) | yes (2-space pretty JSON) | parity |
| Clear validation errors | mixed | yes — truncation, bad length prefix, trailing garbage, unknown tag all rejected with a message | parity / edge |
| 100% client-side / private | some claim it | yes — wasm, runs fully in-browser, offline after load | parity |
| Bidirectional (serialize too) | some | covered by the separate `php-serialize` tool | parity (split across two tools) |
| Three surfaces (chat LLM API + CLI + page) | none | yes | edge (unique to gizza) |

## Gaps considered

- **Side-by-side compare / diff (Unserialize Guru):** out of this tool's single
  input→output model; gizza already ships a dedicated `json-diff` tool, so a compare
  view here would be redundant. Considered, not built.
- **"Tolerant" decoding of mis-encoded / wrong-byte-length strings (DUzun.Me):**
  deliberately NOT adopted. Silently repairing a wrong length prefix hides data
  corruption; our parser instead returns a precise error (which byte failed), which
  is the more correct behavior for inspecting real PHP data. Documented as a feature.
- **Bidirectional UI in one page:** the gizza model is one tool per page; the inverse
  direction already exists as `php-serialize`. Cross-linked in copy, not merged.

## Conclusion

No in-model capability gap remained open. The tool ships at feature parity with the
top competitors on decoding (all PHP serialize types incl. objects, byte-accurate
strings, pretty JSON) and ahead on validation rigor (precise errors rather than silent
repair), class-name preservation, and surface coverage (chat API + CLI + page, where
competitors are page-only). Copy, examples, and FAQ are original.
