import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

// Set a big textarea value directly and fire the same "input" event the driver
// listens to — page.fill on a 10k-line textarea routes through insertText and
// takes minutes (see create-next-tool references/page-patterns.md).
async function setTextareaFast(page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const FIXTURE = [
  'Date,Description,Amount',
  '2024-01-05,WALMART SUPERCENTER,-52.30',
  '2024-01-06,STARBUCKS #1234,-4.50',
  '2024-01-07,SHELL GAS STATION,-48.90',
  '2024-01-08,NETFLIX.COM,-15.99',
  '2024-01-09,WALMART SUPERCENTER,-34.80',
  '2024-01-10,ACME PAYROLL,2000.00',
  '2024-01-11,CITY PARKING,-12.00',
].join('\n');

const SUMMARY = `Spending by category
====================

Category                       Total   Share  Txns
--------------------------------------------------
Groceries                     $87.10   51.7%     2  ████████████████████
Fuel                          $48.90   29.0%     1  ███████████
Subscriptions & Streaming     $15.99    9.5%     1  ████
Transport                     $12.00    7.1%     1  ███
Dining & Takeaway              $4.50    2.7%     1  █
--------------------------------------------------
Total spending               $168.49  100.0%     6
Income                      $2000.00             1
Net cash flow              +$1831.51`;

const ROWS = `Date,Description,Amount,Category
2024-01-05,WALMART SUPERCENTER,-52.30,Groceries
2024-01-06,STARBUCKS #1234,-4.50,Dining & Takeaway
2024-01-07,SHELL GAS STATION,-48.90,Fuel
2024-01-08,NETFLIX.COM,-15.99,Subscriptions & Streaming
2024-01-09,WALMART SUPERCENTER,-34.80,Groceries
2024-01-10,ACME PAYROLL,2000.00,Income
2024-01-11,CITY PARKING,-12.00,Transport`;

test('spending-categorizer default output is summary + categorized CSV', async ({ page }) => {
  await page.goto('/tools/spending-categorizer/');
  await page.fill('#in-data', FIXTURE);
  await expect(page.locator('#tool-output')).toContainText('Total spending', { timeout: 15000 });
  expect(await output(page)).toBe(
    `${SUMMARY}\n\nCategorized transactions\n========================\n${ROWS}`,
  );
});

test('spending-categorizer summary-only output is exact', async ({ page }) => {
  await page.goto('/tools/spending-categorizer/');
  await page.fill('#in-data', FIXTURE);
  await page.selectOption('#in-output', 'summary');
  await expect(page.locator('#tool-output')).toContainText('Net cash flow', { timeout: 15000 });
  expect(await output(page)).toBe(SUMMARY);
});

test('spending-categorizer csv-only output handles debit/credit columns', async ({ page }) => {
  await page.goto('/tools/spending-categorizer/');
  await page.fill(
    '#in-data',
    'Date,Details,Debit,Credit\n01/05/2024,TESCO STORES,23.10,\n01/06/2024,UBER TRIP,11.20,\n01/06/2024,ACME SALARY,,1500.00',
  );
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toContainText('Category', { timeout: 15000 });
  expect(await output(page)).toBe(
    'Date,Description,Amount,Category\n' +
      '01/05/2024,TESCO STORES,-23.10,Groceries\n' +
      '01/06/2024,UBER TRIP,-11.20,Transport\n' +
      '01/06/2024,ACME SALARY,1500.00,Income',
  );
});

test('spending-categorizer custom rules + inverted amounts (non-default checkbox)', async ({
  page,
}) => {
  await page.goto('/tools/spending-categorizer/');
  await page.fill(
    '#in-data',
    'Date,Description,Amount\n2024-01-05,ALDI,52.30\n2024-01-06,STARBUCKS #1234,4.50',
  );
  await page.fill('#in-rules', 'starbucks = Coffee Habit');
  await page.selectOption('#in-output', 'csv');
  await page.check('#in-invert_amount');
  await expect(page.locator('#tool-output')).toContainText('Coffee Habit', { timeout: 15000 });
  expect(await output(page)).toBe(
    'Date,Description,Amount,Category\n' +
      '2024-01-05,ALDI,-52.30,Groceries\n' +
      '2024-01-06,STARBUCKS #1234,-4.50,Coffee Habit',
  );
});

test('spending-categorizer explicit delimiter matrix (comma/semicolon/tab/pipe)', async ({
  page,
}) => {
  await page.goto('/tools/spending-categorizer/');
  await page.selectOption('#in-output', 'csv');
  const cases: Array<[string, string]> = [
    ['comma', 'Description,Amount\nLIDL,-9.99'],
    ['semicolon', 'Description;Amount\nLIDL;-9.99'],
    ['tab', 'Description\tAmount\nLIDL\t-9.99'],
    ['pipe', 'Description|Amount\nLIDL|-9.99'],
  ];
  for (const [delim, data] of cases) {
    await page.selectOption('#in-delimiter', delim);
    await setTextareaFast(page, '#in-data', data);
    await expect(page.locator('#tool-output')).toContainText('Groceries', { timeout: 15000 });
    expect(await output(page), `delimiter=${delim}`).toBe(
      'Description,Amount,Category\nLIDL,-9.99,Groceries',
    );
    await setTextareaFast(page, '#in-data', '');
  }
});

test('spending-categorizer deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'Datum;Beschreibung;Betrag\n15.01.2024;SUPERMARKT KAUFLAND;-42,90\n16.01.2024;STADTWERKE STROM;-65,00\n17.01.2024;REWE MARKT;-23,45',
    rules: 'stadtwerke = Utilities & Phone',
    delimiter: 'semicolon',
    currency: '€',
    output: 'summary',
  });
  await page.goto(`/tools/spending-categorizer/?${params.toString()}`);
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#tool-output')).toContainText('Utilities & Phone', {
    timeout: 15000,
  });
  expect(await output(page)).toBe(`Spending by category
====================

Category             Total   Share  Txns
----------------------------------------
Groceries           €66.35   50.5%     2  ████████████████████
Utilities & Phone   €65.00   49.5%     1  ████████████████████
----------------------------------------
Total spending     €131.35  100.0%     3`);
});

test('spending-categorizer enforces the 10000-row cap exactly', async ({ page }) => {
  await page.goto('/tools/spending-categorizer/');
  await page.selectOption('#in-output', 'summary');
  const rows: string[] = ['Description,Amount'];
  for (let i = 0; i < 10000; i++) rows.push(`Merchant ${i},-1.00`);
  await setTextareaFast(page, '#in-data', rows.join('\n'));
  await expect(page.locator('#tool-output')).toContainText('Total spending', { timeout: 30000 });
  // All 10000 rows land in Other: exactly at the cap must succeed.
  expect(await output(page)).toContain('Total spending  $10000.00  100.0%  10000');
  // One row over the cap must fail with the advertised bound.
  rows.push('One More,-1.00');
  await setTextareaFast(page, '#in-data', rows.join('\n'));
  await expect(page.locator('#tool-output')).toContainText(
    'too many rows: 10001 (max 10000 per run)',
    { timeout: 30000 },
  );
});

test('spending-categorizer explains a missing description column', async ({ page }) => {
  await page.goto('/tools/spending-categorizer/');
  await page.fill('#in-data', 'Foo,Bar\n1,2');
  await expect(page.locator('#tool-output')).toContainText(
    'could not find a description column',
    { timeout: 15000 },
  );
});
