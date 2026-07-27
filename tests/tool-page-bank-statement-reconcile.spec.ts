import { test, expect } from './fixtures';

const STATEMENT = 'date,amount,memo\n2024-01-02,-42.50,COFFEE SHOP\n2024-01-05,-1200.00,RENT PAYMENT\n2024-01-06,-9.99,APP STORE';
const LEDGER = 'date,amount,memo\n2024-01-02,-42.50,Coffee Shop downtown\n2024-01-05,-1200.00,January rent\n2024-01-07,-15.00,Office snacks';

const MARKDOWN_OUTPUT = `# Bank statement reconciliation

- Statement transactions: 3
- Ledger transactions: 3
- Matched: 2
- Suggested matches: 0
- Unmatched statement: 1
- Unmatched ledger: 1

## Matched (2)

| Statement date | Amount | Statement memo | Ledger date | Ledger memo | Memo % | Δ days | Δ amount |
|---|---|---|---|---|---|---|---|
| 2024-01-02 | -42.5 | COFFEE SHOP | 2024-01-02 | Coffee Shop downtown | 80 | 0 | 0 |
| 2024-01-05 | -1200 | RENT PAYMENT | 2024-01-05 | January rent | 50 | 0 | 0 |

## Suggested matches (0)

_None._

## Unmatched statement (1)

| Date | Amount | Memo |
|---|---|---|
| 2024-01-06 | -9.99 | APP STORE |

## Unmatched ledger (1)

| Date | Amount | Memo |
|---|---|---|
| 2024-01-07 | -15 | Office snacks |`;

const JSON_OUTPUT = `{
  "matched": [
    {
      "amount_diff": 0.0,
      "date_diff_days": 0,
      "ledger": {
        "amount": -42.5,
        "date": "2024-01-02",
        "memo": "Coffee Shop downtown"
      },
      "memo_similarity": 80,
      "statement": {
        "amount": -42.5,
        "date": "2024-01-02",
        "memo": "COFFEE SHOP"
      }
    }
  ],
  "suggested_matches": [],
  "summary": {
    "ledger_count": 1,
    "matched": 1,
    "statement_count": 1,
    "suggested_matches": 0,
    "unmatched_ledger": 0,
    "unmatched_statement": 0
  },
  "unmatched_ledger": [],
  "unmatched_statement": []
}`;

test('bank-statement-reconcile renders exact markdown reconciliation output', async ({ page }) => {
  await page.goto('/tools/bank-statement-reconcile/');
  await page.fill('#in-statement_csv', STATEMENT);
  await page.fill('#in-ledger_csv', LEDGER);
  await page.fill('#in-memo_threshold', '50');
  await page.selectOption('#in-output', 'markdown');

  await expect(page.locator('#tool-output')).toHaveText(MARKDOWN_OUTPUT, { timeout: 15000 });
});

test('bank-statement-reconcile supports JSON output and strict tolerances', async ({ page }) => {
  await page.goto('/tools/bank-statement-reconcile/');
  await page.fill('#in-statement_csv', 'date,amount,memo\n2024-01-02,-42.50,COFFEE SHOP');
  await page.fill('#in-ledger_csv', 'date,amount,memo\n2024-01-02,-42.50,Coffee Shop downtown');
  await page.fill('#in-date_tolerance_days', '0');
  await page.fill('#in-amount_tolerance', '0');
  await page.fill('#in-memo_threshold', '80');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(JSON_OUTPUT, { timeout: 15000 });
});

test('bank-statement-reconcile deep-link pre-fills params and auto-runs JSON', async ({ page }) => {
  const params = new URLSearchParams({
    statement_csv: 'date,amount,memo\n2024-01-02,-42.50,COFFEE SHOP',
    ledger_csv: 'date,amount,memo\n2024-01-02,-42.50,Coffee Shop downtown',
    stmt_date: 'date',
    stmt_amount: 'amount',
    stmt_memo: 'memo',
    ledger_date: 'date',
    ledger_amount: 'amount',
    ledger_memo: 'memo',
    date_tolerance_days: '0',
    amount_tolerance: '0',
    memo_threshold: '80',
    delimiter: 'comma',
    output: 'json',
  });

  await page.goto(`/tools/bank-statement-reconcile/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-date_tolerance_days')).toHaveValue('0');
  await expect(page.locator('#tool-output')).toHaveText(JSON_OUTPUT, { timeout: 15000 });
});
