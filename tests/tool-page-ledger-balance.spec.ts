import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SAMPLE = '2024-01-05 * Groceries\n    Expenses:Food:Groceries   $45.20\n    Assets:Bank:Checking\n\n2024-01-10 Salary\n    Assets:Bank:Checking      $2,000.00\n    Income:Salary            $-2,000.00\n\n2024-02-01 ! Coffee\n    Expenses:Food:Coffee      $4.80\n    Assets:Bank:Checking     $-4.80';

const TREE_EXPECTED = ' $1950.00  Assets\n $1950.00    Bank\n $1950.00      Checking\n   $50.00  Expenses\n   $50.00    Food\n    $4.80      Coffee\n   $45.20      Groceries\n$-2000.00  Income\n$-2000.00    Salary\n---------\n    $0.00';

test('ledger-balance renders the exact tree balance report', async ({ page }) => {
  await page.goto('/tools/ledger-balance/');
  await page.fill('#in-journal', SAMPLE);
  await expect(page.locator('#tool-output')).toContainText('Assets', { timeout: 15000 });
  expect(await output(page)).toBe(TREE_EXPECTED);
});

test('ledger-balance supports select controls and a non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/ledger-balance/');
  await page.fill('#in-journal', SAMPLE);
  await page.selectOption('#in-layout', 'flat');
  await page.selectOption('#in-sort', 'amount');
  await page.selectOption('#in-output_format', 'csv');
  await page.fill('#in-account_filter', 'expenses, not:coffee');
  await page.uncheck('#in-show_total');
  await expect(page.locator('#tool-output')).toContainText('Expenses:Food:Groceries,$,45.20', { timeout: 15000 });
  expect(await output(page)).toBe('account,commodity,amount\nExpenses:Food:Groceries,$,45.20');
});

test('ledger-balance deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    journal: SAMPLE,
    depth: '1',
    layout: 'flat',
    begin: '2024-01-01',
    end: '2024-03-01',
    status: 'pending',
    output_format: 'markdown',
  });
  await page.goto(`/tools/ledger-balance/?${params.toString()}`);
  await expect(page.locator('#in-journal')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-depth')).toHaveValue('1');
  await expect(page.locator('#in-status')).toHaveValue('pending');
  await expect(page.locator('#tool-output')).toContainText('| Account | Balance |', { timeout: 15000 });
  expect(await output(page)).toContain('| Expenses | $4.80 |');
});
