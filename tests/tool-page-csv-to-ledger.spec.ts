import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SIGNED_CSV = 'Date,Description,Amount\n2024-01-15,Starbucks Coffee,-4.50\n2024-01-16,ACME Payroll,2000.00';
const SIGNED_EXPECTED = '2024-01-15 Starbucks Coffee\n    Expenses:Food           $4.50\n    Assets:Bank:Checking    $-4.50\n\n2024-01-16 ACME Payroll\n    Income:Salary           $-2000.00\n    Assets:Bank:Checking    $2000.00';

test('csv-to-ledger converts signed bank CSV to exact ledger output', async ({ page }) => {
  await page.goto('/tools/csv-to-ledger/');
  await page.fill('#in-data', SIGNED_CSV);
  await expect(page.locator('#tool-output')).toContainText('Assets:Bank:Checking', { timeout: 15000 });
  expect(await output(page)).toBe(SIGNED_EXPECTED);
});

test('csv-to-ledger honors debit/credit columns and hledger compact output', async ({ page }) => {
  await page.goto('/tools/csv-to-ledger/');
  await page.fill('#in-data', 'Date,Payee,Debit,Credit\n01/15/2024,Rent,1500.00,\n01/20/2024,Refund,,25.00');
  await page.selectOption('#in-output_format', 'hledger');
  await expect(page.locator('#tool-output')).toContainText('Expenses:Rent', { timeout: 15000 });
  expect(await output(page)).toBe('2024-01-15 Rent\n    Expenses:Rent           $1500.00\n    Assets:Bank:Checking\n\n2024-01-20 Refund\n    Income:Refunds          $-25.00\n    Assets:Bank:Checking');
});

test('csv-to-ledger supports non-default checkbox and custom account rules', async ({ page }) => {
  await page.goto('/tools/csv-to-ledger/');
  await page.fill('#in-data', 'Date,Description,Amount\n2024-01-15,STARBUCKS #123,6.00');
  await page.fill('#in-account_rules', 'starbucks = Expenses:Coffee:Treats');
  await page.check('#in-invert_amount');
  await expect(page.locator('#tool-output')).toContainText('Expenses:Coffee:Treats', { timeout: 15000 });
  expect(await output(page)).toBe('2024-01-15 STARBUCKS #123\n    Expenses:Coffee:Treats    $6.00\n    Assets:Bank:Checking      $-6.00');
});

test('csv-to-ledger deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'Date;Description;Amount\n15.01.2024;Supermarkt REWE;-42,90',
    date_format: 'dmy',
    delimiter: 'semicolon',
    currency: '€',
    output_format: 'hledger',
    account_rules: 'rewe = Expenses:Groceries',
  });
  await page.goto(`/tools/csv-to-ledger/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue('Date;Description;Amount\n15.01.2024;Supermarkt REWE;-42,90', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('hledger');
  await expect(page.locator('#tool-output')).toContainText('Expenses:Groceries', { timeout: 15000 });
  expect(await output(page)).toBe('2024-01-15 Supermarkt REWE\n    Expenses:Groceries      €42.90\n    Assets:Bank:Checking');
});
