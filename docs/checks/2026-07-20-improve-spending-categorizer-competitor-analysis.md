# spending-categorizer — competitor analysis (2026-07-20)

New-tool build scan (done BEFORE implementing). One WebSearch for "bank transaction CSV
categorize spending by category tool"; skimmed the top 3 reachable competitor tools.
Paraphrased only — no competitor copy, branding, or trademarks reproduced.

Dup check first: `csv-to-ledger` shares the CSV-parsing + keyword-guessing shape but outputs
double-entry ledger-cli/hledger journal text for plaintext-accounting users; this tool outputs
a per-category spending summary + a categorized CSV for budget review. Different output,
different audience — not a dup (same reasoning as csv-group-by vs csv-stats coexisting).

## Competitors skimmed

1. **Skwad — free bank transaction categorizer** (skwad.app/free-bank-transaction-categorizer)
   Paste transactions, AI-classifies into standard categories (groceries, dining, transport,
   subscriptions). Custom categories behind signup; daily row limits.
2. **Expense Categorizer** (expensecategorizer.com) Upload CSV/PDF statements; vendor→category
   mappings configurable before upload and remembered in-browser; output is a clean categorized
   CSV "ready for bookkeeping"; categories like fuel, software, meals, insurance.
3. **Bank CSV Categorizer** (bankcsvcategorizer.com) Upload bank CSV (Date/Description/Amount);
   auto-categorizes into Groceries, Income, Subscriptions, Transit, Dining, Utilities, Rent,
   Shopping; per-row category table in a dashboard; export to CSV/Sheets/Excel; "categorization
   rules" and per-category budget caps flagged as coming-soon.

Also surfaced (not skimmed in depth): receiptsai.com and csvmoney.com — both AI-based
categorizers with CSV/Excel export; receiptsai splits expenses / income / transfers / fees.

## Table stakes → decision

| Capability | Competitors | Tag | Where it landed |
|---|---|---|---|
| Paste or upload a bank/card CSV | all 3 | in-model | `data` textarea (paste), header row required |
| Auto-detect Date / Description / Amount columns | bankcsvcategorizer | in-model | auto-detect + explicit `*_column` overrides |
| Separate Debit/Credit column statements | common bank exports | in-model | `debit_column` / `credit_column` (auto-detected too) |
| Built-in category set (groceries, dining, transit, subscriptions, utilities, rent, shopping, fuel, insurance, income…) | all 3 | in-model | 16 built-in categories, ordered merchant-keyword table |
| Custom vendor→category rules | expensecategorizer (mappings), bankcsvcategorizer (coming soon), skwad (paid) | in-model | `rules` param: one `keyword = Category` per line, checked before built-ins |
| Per-row categorized table | all 3 | in-model | `output=csv` (and the rows section of `both`) |
| Category totals / summary view | all 3 dashboards | in-model | `output=summary`: totals, share %, txn counts, text bar chart |
| Export categorized CSV | expensecategorizer, bankcsvcategorizer | in-model | `output=csv` + the page's automatic Download link for text output |
| Income vs expense split (income listed apart from spending) | receiptsai, bankcsvcategorizer | in-model | sign-based: income keywords + positive-amount fallback → `Income`; summary shows Total spending / Income / Net cash flow |
| Spending-as-positive exports | common bank quirk | in-model | `invert_amount` checkbox |
| Non-comma delimiters / EU decimal commas | EU bank exports | in-model | `delimiter` enum (auto-sniff default), amount parser handles `1.234,56`, `(12.34)`, `DR/CR` |
| Charts/dashboard | bankcsvcategorizer | partially in-model | proportional text bar per category in the summary; for real charts the toolkit already ships csv-chart-generator |
| AI/ML classification ("95% accuracy") | skwad, csvmoney, receiptsai | out-of-model | needs an ML model; gizza is pure-Rust wasm. Keyword matching + user rules is the deterministic equivalent; stated on the page |
| PDF statement input | expensecategorizer | out-of-model | separate concern (PDF extraction tools exist in the toolkit); this tool is CSV-only, stated on page |
| Vendor-mapping persistence across visits | expensecategorizer | out-of-model | stateless tool; deep-link URL params (incl. `rules`) are the shareable equivalent |
| Multi-file merge + month filter | expensecategorizer | out-of-model | single paste per run; csv-merge / csv-filter cover pre-processing |
| Budget caps per category | bankcsvcategorizer (coming soon) | out-of-model | not built by the competitor either; deferred |

## UX control patterns observed → ours

- Drag-drop/upload + dashboards (all 3) → paste textarea + instant text output (our pure-tool
  pattern); Download link comes free with `format = "text"`.
- Numbered step flows / FAQ accordions → page `content.md` worked example + `<details>` FAQs.
- Preset/vendor-mapping management buttons → `[[example]]` preset chips: signed-amount
  statement, debit/credit statement, custom-rules + semicolon-delimiter example.
- Select controls for enums (`output`, `delimiter`), checkbox for `invert_amount`, multiline
  textareas for `data` and `rules`.

## Descriptor designed from the scan (from the start)

`data` (required), `description_column`, `amount_column`, `debit_column`, `credit_column`,
`date_column` (all blank = auto-detect), `rules` (keyword = Category lines),
`output` enum both|summary|csv (default both), `currency` (symbol prefix / code suffix),
`delimiter` enum auto|comma|semicolon|tab|pipe, `invert_amount` boolean. Cap: 10 000 rows
(matches csv-to-ledger, advertised in the schema + page).

Keyword matching design note: single-word built-in keywords match on token-prefix (so `rent`
hits RENT/RENTAL but not PARENT/CURRENT; `fee` not COFFEE; `interest` not PINTEREST); keywords
with spaces/punctuation substring-match; user rules substring-match (documented). Ordered
table puts specific merchants before generic words (uber eats/eats → Dining before uber →
Transport; amazon prime → Subscriptions before amazon → Shopping).
