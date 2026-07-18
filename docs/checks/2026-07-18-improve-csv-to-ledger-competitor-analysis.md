# csv-to-ledger — competitor analysis (2026-07-18)

Scan of the top real tools that turn a bank/credit-card CSV export into
ledger-cli / hledger double-entry journal entries. All copy below is
**paraphrased** — no competitor wording, branding, or trademarks copied.

## Competitors reviewed

| # | Tool | URL | Shape |
| - | ---- | --- | ----- |
| 1 | hledger `import` / CSV rules | https://hledger.org/import-csv.html | Built-in CLI; a `.csv.rules` file maps columns → fields and matches descriptions → accounts. Defaults unmatched postings to `income:unknown` / `expenses:unknown`. |
| 2 | ledger-bank-import (marbu) | https://github.com/marbu/ledger-bank-import | CLI converting bank CSV → ledger-cli; per-bank column config + regex account rules. |
| 3 | csv2ledger (joostkremers, Emacs) | https://codeberg.org/joostkremers/csv2ledger | Matches payee/description fields against an account-matchers file to auto-pick the balancing account; configurable match fields. |
| 4 | reckon | https://github.com/cantino/reckon | Interactive CSV→ledger; learns/guesses the counter-account, handles separate in/out or one signed amount, date parsing. |
| 5 | hledger-flow (apauley) | https://github.com/apauley/hledger-flow | Workflow layer over hledger import: automated statement import + classification across many accounts. |

## Table-stakes (must-have — all shipped in this tool)

- **Column mapping, case-insensitive** — let the user name the date /
  description(payee) / amount columns, and auto-detect common headers
  (`Date`, `Description`/`Payee`/`Memo`, `Amount`) when unspecified. ✅
- **Signed-amount OR separate debit/credit columns** — banks export either a
  single signed `Amount` or split `Debit`/`Credit` (a.k.a. Withdrawal/Deposit,
  Money Out/In). Support both and infer the sign. ✅
- **Sign convention + invert** — money out → expense leg, money in → income leg;
  a toggle to flip when a bank exports outflows as positive. ✅
- **Default accounts** — mirror hledger's `expenses:unknown` / `income:unknown`
  fallback, plus a configurable asset/bank account for the other leg. ✅
- **Description → account rules** — the core value: substring/keyword rules that
  pick the balancing account (csv2ledger's matchers, hledger rules `if` blocks).
  Support user rules **plus** built-in keyword heuristics (groceries, transport,
  utilities, salary…). ✅
- **Date normalisation** — accept US `MM/DD/YYYY`, EU `DD/MM/YYYY`, ISO, 2-digit
  years, and `-`/`/`/`.` separators + month names; emit ISO `YYYY-MM-DD`. ✅
- **Amount normalisation** — strip currency symbols and thousands separators,
  `(1,234.56)` parentheses = negative, US vs EU decimal separator. ✅
- **ledger + hledger output** — both accept the same syntax; offer explicit
  balancing amount (ledger, safest) vs an inferred/omitted balancing posting
  (hledger-idiomatic, compact). ✅
- **Delimiter choice** — comma/semicolon/tab/pipe, auto-sniffed. ✅
- **Commodity/currency** — symbol prefix (`$4.50`) vs code suffix (`4.50 USD`). ✅

## Decisions for the gizza tool

- **Browser-local, paste-in, zero-config path.** Competitors are CLIs needing a
  rules file on disk; our differentiator is a single paste box that produces a
  usable journal immediately, with optional rules for power users. Auto-detect
  headers so the common case needs no configuration.
- **Built-in heuristics beyond a blank default.** hledger defaults everything to
  `expenses:unknown`; we ship a keyword table (groceries/food/transport/
  utilities/subscriptions/rent/health/fees on the expense side; salary/interest/
  refund on the income side) so first output is already mostly categorised, then
  user `account_rules` override.
- **Aligned, ready-to-append output** with a blank line between transactions and
  amount columns padded — matches what both tools pretty-print.

## Out-of-model (not built — needs server/state)

- **Idempotent dedup across repeated imports** (hledger `import` skips already-seen
  transactions) — needs persistent state of prior imports; a stateless paste tool
  can't track that.
- **Learning/interactive account guessing** (reckon learns from your past
  choices) — needs a trained model / prior journal; out of a pure paste tool.
- **Multi-account workflow orchestration** (hledger-flow) — needs a filesystem
  tree of statements.
