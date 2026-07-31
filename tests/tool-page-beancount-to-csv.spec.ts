import { test, expect } from './fixtures';

const journal = `2024-01-15 * "Starbucks" "Morning coffee"
  Expenses:Food:Coffee    4.50 USD
  Assets:Bank:Checking   -4.50 USD`;

const csv = `date,flag,payee,narration,account,amount,currency
2024-01-16,*,,Grocery Store,Expenses:Groceries,25.00,$
2024-01-16,*,,Grocery Store,Assets:Bank:Checking,-25.00,$`;

test('beancount-to-csv flattens journal postings to CSV', async ({ page }) => {
  await page.goto('/tools/beancount-to-csv/');
  await page.selectOption('#in-direction', 'to-csv');
  await page.fill('#in-input', journal);
  await page.selectOption('#in-delimiter', 'comma');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('date,flag,payee,narration,account,amount,currency,cost,price,comment', { timeout: 15000 });
  await expect(out).toContainText('2024-01-15,*,Starbucks,Morning coffee,Expenses:Food:Coffee,4.50,USD,,,');
  await expect(out).toContainText('2024-01-15,*,Starbucks,Morning coffee,Assets:Bank:Checking,-4.50,USD,,,');
});

test('beancount-to-csv rebuilds Ledger journal from CSV', async ({ page }) => {
  await page.goto('/tools/beancount-to-csv/');
  await page.selectOption('#in-direction', 'from-csv');
  await page.selectOption('#in-journal_format', 'ledger');
  await page.selectOption('#in-delimiter', 'comma');
  await page.fill('#in-input', csv);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2024-01-16 * Grocery Store', { timeout: 15000 });
  await expect(out).toContainText('Expenses:Groceries  $25.00');
  await expect(out).toContainText('Assets:Bank:Checking  $-25.00');
});

test('beancount-to-csv supports deep-linked semicolon CSV output', async ({ page }) => {
  const params = new URLSearchParams({
    direction: 'to-csv',
    input: '2024-04-01 * "Refund"\n    Assets:Cash   (20.00) USD',
    journal_format: 'beancount',
    delimiter: 'semicolon',
  });
  await page.goto(`/tools/beancount-to-csv/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('date;flag;payee;narration;account;amount;currency;cost;price;comment', { timeout: 15000 });
  await expect(out).toContainText('2024-04-01;*;;Refund;Assets:Cash;-20.00;USD;;;');
});
