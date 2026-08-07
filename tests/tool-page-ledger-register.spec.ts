import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SAMPLE = '2024-01-05 * Groceries\n    Expenses:Food:Groceries   $45.20\n    Assets:Bank:Checking\n\n2024-01-10 Salary\n    Assets:Bank:Checking      $2,000.00\n    Income:Salary            $-2,000.00\n\n2024-02-01 ! Coffee\n    Expenses:Food:Coffee      $4.80\n    Assets:Bank:Checking     $-4.80';

const CHECKING_EXPECTED = '2024-01-05 Groceries           Assets:Bank:Checking            $-45.20   $-45.20\n2024-01-10 Salary              Assets:Bank:Checking           $2000.00  $1954.80\n2024-02-01 Coffee              Assets:Bank:Checking             $-4.80  $1950.00';

test('ledger-register renders an exact checking register with running total', async ({ page }) => {
  await page.goto('/tools/ledger-register/');
  await page.fill('#in-journal', SAMPLE);
  await page.fill('#in-account_filter', 'checking');
  await expect(page.locator('#tool-output')).toContainText('Assets:Bank:Checking', { timeout: 15000 });
  expect(await output(page)).toBe(CHECKING_EXPECTED);
});

test('ledger-register supports select controls and non-default checkbox states', async ({ page }) => {
  await page.goto('/tools/ledger-register/');
  await page.fill('#in-journal', SAMPLE);
  await page.fill('#in-account_filter', 'checking');
  await page.check('#in-related');
  await page.check('#in-invert');
  await page.selectOption('#in-output_format', 'csv');
  await page.fill('#in-limit', '2');
  await page.selectOption('#in-limit_from', 'last');
  await expect(page.locator('#tool-output')).toContainText('date,description,account,commodity,amount,total', { timeout: 15000 });
  const text = await output(page);
  expect(text).toContain('Income:Salary,$,2000.00,1954.80');
  expect(text).toContain('Expenses:Food:Coffee,$,-4.80,1950.00');
});

test('ledger-register deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    journal: SAMPLE,
    account_filter: 'checking',
    begin: '2024-02-01',
    running_total: 'historical',
    status: 'pending',
    output_format: 'markdown',
  });
  await page.goto(`/tools/ledger-register/?${params.toString()}`);
  await expect(page.locator('#in-journal')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-running_total')).toHaveValue('historical');
  await expect(page.locator('#in-status')).toHaveValue('pending');
  await expect(page.locator('#tool-output')).toContainText('| Date | Description | Account | Amount | Total |', { timeout: 15000 });
  expect(await output(page)).toContain('| 2024-02-01 | Coffee | Assets:Bank:Checking | $-4.80 | $-4.80 |');
});
