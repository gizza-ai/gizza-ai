# random-token-generator — competitor analysis (2026-06-21)

Tool: generate cryptographically random tokens / API keys / secrets with a
configurable length and character set. Surfaces verified: chat block (wafer
build OK, instantiates; `cargo test --workspace` green incl. drift-guard), CLI
(`gizza tool random-token-generator …` across hex / base64url / safe / custom /
error paths), page (Playwright 2/2 — default hex and `?length=24&count=3&charset=base64url`
deep-link).

## Top competitors surveyed

1. **RandomKeygen (randomkeygen.com)** — one-shot page that emits fixed buckets
   of keys (memorable, strong, 256-bit WEP/WPA, CodeIgniter/Laravel-style). No
   user-set length or alphabet; you copy whichever pre-sized bucket fits.
2. **generate-secret.vercel.app** — minimal "generate a 32/64-byte secret"
   utility aimed at env vars (`SESSION_SECRET`), hex/base64 output, length presets.
3. **Browserling "Generate Random Strings"** — configurable length, count, and
   character classes (lower/upper/digits/symbols), batch output, in-page.
4. **IT-Tools "Token generator"** — toggles for uppercase / lowercase / numbers /
   symbols, a length slider, copy button, regenerate; client-side.
5. **1Password / Bitwarden generator** — password+token oriented, length slider,
   character-class toggles, "avoid ambiguous characters" option, entropy hinting.

## Capability diff (us vs. field)

| Capability                              | Us | Typical competitor |
|-----------------------------------------|----|---------------------|
| Configurable length                     | ✅ | ✅ |
| Multiple character-set presets          | ✅ (hex, hex-upper, base64url, alphanumeric, alphabetic, numeric, safe) | partial (most only class toggles) |
| Custom alphabet                         | ✅ | rare |
| Batch / count output                    | ✅ (1–1000) | some (Browserling) |
| "Avoid ambiguous characters" set        | ✅ (`safe`) | 1Password/Bitwarden only |
| Cryptographic RNG, no modulo bias       | ✅ (rejection sampling) | usually unstated |
| Per-token entropy reported              | ✅ (bits + alphabet size) | rare |
| Runs fully client-side / nothing uploaded | ✅ | mixed (some are server-side) |
| Usable from CLI + LLM chat, not just a page | ✅ (3 surfaces) | ❌ (page only) |

## Gaps closed in this build

- **Preset breadth:** shipped seven presets (incl. URL-safe base64 and a no-
  ambiguous `safe` set) rather than only character-class toggles — covers the
  IT-Tools/Browserling/1Password feature set in one parameter.
- **Custom alphabet:** `custom_chars` overrides the preset (dedups, requires ≥2
  distinct), beating most competitors that lock you to fixed classes.
- **Batch generation:** `count` (1–1000) matches Browserling's batch output.
- **Honest entropy:** output reports per-token entropy in bits and the alphabet
  size, which competitors generally omit; computed from the actual alphabet.
- **Correctness:** uniform sampling via rejection (no modulo bias) for non-
  power-of-two alphabets — a subtlety most generators get wrong or don't claim.
- **Copy/SEO:** page copy explains each charset, recommends ~128-bit defaults
  (32 hex / 22 base64url), and stresses local-only generation.

## Out-of-model / deliberately not built

- **UUID / GUID and JWT/secret-format presets** — distinct tools (a UUID has a
  fixed RFC-4122 layout, not a free-length token); would overlap a future
  `uuid-generator` block rather than belong here.
- **Copy-to-clipboard button / regenerate animation** — page-shell UX owned by
  the generator template, not per-tool.
- **WPA/WEP-style fixed buckets (RandomKeygen)** — superseded by arbitrary
  length + charset, which expresses any bucket on demand.

No competitor copy, branding, or trademarks were used.
