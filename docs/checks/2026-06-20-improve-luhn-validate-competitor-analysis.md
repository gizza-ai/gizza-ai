# luhn-validate — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/luhn-validate` — validate a number against the Luhn (mod-10)
check-digit algorithm (credit/debit cards, IMEI, ID schemes). Chat + CLI + page
(pure-string, no deps).

## What competitors do

- **Online Luhn / card validators** (dcode, validatecreditcard sites, gchq
  CyberChef "Luhn") — paste a number, get valid/invalid. Strengths: simple.
  Weaknesses: many **send the number to a server** (you should never paste a real
  card into a remote validator), ad-heavy, and most give only a yes/no.
- **`echo ... | luhn` / language one-liners** — local but require code/CLI.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: page
   in-browser, CLI headless, chat Service Worker. Critical for a tool people feed
   card/IMEI numbers into.
2. **More than yes/no.** Returns the cleaned digits + length, **the check digit
   that would make it valid** (catches a single-digit typo, or completes a
   number), and a best-effort **card brand** (Visa/Mastercard/Amex/Discover/JCB/
   Diners) from the length + IIN prefix.
3. **Forgiving input.** Spaces, dashes, tabs, and underscores are ignored, so
   `4242 4242 4242 4242` and `4242-4242-...` both work; non-digit garbage errors
   clearly.
4. **Honest about scope** (see notes) — it's a checksum, not a card-realness
   check.
5. **Three surfaces + deep-links.**

## Honest scope

- Luhn is a **typo checksum**, not validation that a card is real, active, or
  issued — the page/description say so explicitly. No network/BIN lookup.
- Brand detection is prefix/length heuristic (covers the major networks); unusual
  co-branded ranges may not be labeled.

## Tests

6 core unit tests: a valid Visa test card (`4242…`, brand=Visa), an off-by-one
invalid number with the correct `expected_check_digit`, a known valid 15-digit
**IMEI** (`490154203237518`, no brand), Amex + Mastercard brand detection,
spaces/dashes ignored, and error cases (empty / letters / too short). Plus the
block drift-guard schema test. CLI + Playwright (valid Visa via fill; invalid via
deep-link showing the corrected digit) verified — see commit.
