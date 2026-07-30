import { test, expect } from './fixtures';

const sample = `Item, Amount, Type, Category
Home, 320000, asset, Real Estate
Brokerage, 80000, asset, Investments
Mortgage, 240000, liability, Real Estate
Credit Card, 4000, liability, Credit Card`;

async function outputText(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Net worth:', { timeout: 15_000 });
  return ((await out.textContent()) ?? '').trim();
}

test('net-worth-tracker computes an exact sample balance sheet', async ({ page }) => {
  await page.goto('/tools/net-worth-tracker/');
  await page.fill('#in-input', sample);
  await page.selectOption('#in-sort', 'value');
  await page.fill('#in-currency', '$');

  expect(await outputText(page)).toBe(`Net worth: $156,000.00   (Assets $400,000.00 − Liabilities $244,000.00)

Assets — $400,000.00 total across 2 items
  Real Estate  $320,000.00   80.00%  ███████████████████·····  (1 item)
  Investments   $80,000.00   20.00%  █████···················  (1 item)

Liabilities — $244,000.00 total across 2 items
  Real Estate  $240,000.00   98.36%  ████████████████████████  (1 item)
  Credit Card    $4,000.00    1.64%  ························  (1 item)

Debt-to-asset ratio: 61.00%   (you own 39.00% of your assets)`);
});

test('net-worth-tracker supports alphabetical category sort and currency prefixes', async ({ page }) => {
  await page.goto('/tools/net-worth-tracker/');
  await page.fill('#in-input', `Savings, 12000, Cash
Investments, 30000, Investments
Student Loan, -22000, Education
Credit Card, -4000, Credit Card`);
  await page.selectOption('#in-sort', 'label');
  await page.fill('#in-currency', '£');

  const text = await outputText(page);
  expect(text).toContain('Net worth: £16,000.00   (Assets £42,000.00 − Liabilities £26,000.00)');
  expect(text).toContain('  Cash         £12,000.00');
  expect(text).toContain('  Investments  £30,000.00');
  expect(text).toContain('  Credit Card   £4,000.00');
  expect(text).toContain('  Education    £22,000.00');
});

test('net-worth-tracker deep-links spreadsheet-style input', async ({ page }) => {
  const input = `Checking\t8000\tasset\tCash
Brokerage\t42000\tasset\tInvestments
Auto Loan\t9000\tliability\tVehicles`;
  const qs = new URLSearchParams({ input, sort: 'value', currency: '$' });
  await page.goto(`/tools/net-worth-tracker/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(input);
  await expect(page.locator('#in-sort')).toHaveValue('value');
  const text = await outputText(page);
  expect(text).toContain('Net worth: $41,000.00   (Assets $50,000.00 − Liabilities $9,000.00)');
  expect(text).toContain('Debt-to-asset ratio: 18.00%');
});
