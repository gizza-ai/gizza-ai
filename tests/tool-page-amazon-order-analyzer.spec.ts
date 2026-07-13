import { test, expect } from './fixtures';

const CLASSIC = [
  'Order Date,Order ID,Title,Category,Quantity,Item Total',
  '2024-01-05,111-1111111-1111111,USB-C Cable,Electronics,2,$9.99',
  '2024-01-20,111-1111111-1111112,Coffee Beans,Grocery,1,$18.50',
  '2024-02-03,111-1111111-1111113,USB-C Cable,Electronics,1,$9.99',
].join('\n');

const REQUEST_MY_DATA = [
  'Order Date,Order ID,Product Name,Quantity,Total Owed',
  '2024-03-01,A1,Notebook,3,$12.00',
  '2024-03-15,A2,Desk Lamp,1,$32.50',
].join('\n');

test('amazon-order-analyzer summarizes total, months, top items, and categories', async ({ page }) => {
  await page.goto('/tools/amazon-order-analyzer/');
  await page.fill('#in-csv', CLASSIC);
  await page.fill('#in-top', '5');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('$38.48 across 3 items in 3 orders', { timeout: 15000 });
  await expect(out).toContainText('| 2024-01 | 2 | $28.49 |');
  await expect(out).toContainText('| 2024-02 | 1 | $9.99 |');
  await expect(out).toContainText('| $19.98 | 3 | USB-C Cable |');
  await expect(out).toContainText('| Electronics | 2 | $19.98 | 51.9% |');
});

test('amazon-order-analyzer supports json output and deep links', async ({ page }) => {
  const params = new URLSearchParams({ csv: CLASSIC, output: 'json', top: '2' });
  await page.goto(`/tools/amazon-order-analyzer/?${params.toString()}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"total_spend": 38.48', { timeout: 15000 });
  await expect(out).toContainText('"month": "2024-01"');
  await expect(out).toContainText('"item": "USB-C Cable"');
});

test('amazon-order-analyzer handles Request My Data exports without categories', async ({ page }) => {
  await page.goto('/tools/amazon-order-analyzer/');
  await page.fill('#in-csv', REQUEST_MY_DATA);
  await page.fill('#in-top', '10');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('$44.50 across 2 items in 2 orders', { timeout: 15000 });
  await expect(out).toContainText('| 2024-03 | 2 | $44.50 |');
  await expect(out).toContainText('No Category column was found');
});
