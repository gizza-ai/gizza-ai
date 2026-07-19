import { test, expect } from './fixtures';

const SAMPLE = 'Order 42 shipped for $1,299.99 on 2024. Weight 3.5kg, ref -7.';

test('extract-numbers-from-text pulls every number with defaults', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/');
  await page.fill('#in-text', SAMPLE);
  const out = page.locator('#tool-output');
  // Default delimiter is newline, mode=all, original order.
  await expect(out).toContainText('42', { timeout: 15000 });
  await expect(out).toContainText('1,299.99');
  await expect(out).toContainText('2024');
  await expect(out).toContainText('3.5');
  await expect(out).toContainText('-7');
});

test('extract-numbers-from-text integers-only mode drops decimals', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/');
  await page.fill('#in-text', '1 2.5 3 4.0 5e2');
  await page.selectOption('#in-mode', 'integers');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('5e2', { timeout: 15000 });
  await expect(out).not.toContainText('2.5');
  await expect(out).not.toContainText('4.0');
});

test('extract-numbers-from-text decimals-only mode keeps only decimals', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/');
  await page.fill('#in-text', '1 2.5 3 4.0 5e2');
  await page.selectOption('#in-mode', 'decimals');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2.5', { timeout: 15000 });
  await expect(out).toContainText('4.0');
});

test('extract-numbers-from-text sorts descending with comma delimiter', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/');
  await page.fill('#in-text', '10 -3 2.5 100');
  await page.selectOption('#in-sort', 'descending');
  await page.selectOption('#in-delimiter', 'comma');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('100, 10, 2.5, -3', { timeout: 15000 });
});

test('extract-numbers-from-text de-duplicates and appends stats', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/');
  await page.fill('#in-text', 'a 1 b 2 c 3 d 3 e 1');
  await page.check('#in-unique');
  await page.check('#in-stats');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Count: 3', { timeout: 15000 });
  await expect(out).toContainText('Sum: 6');
  await expect(out).toContainText('Min: 1');
  await expect(out).toContainText('Max: 3');
  await expect(out).toContainText('Average: 2');
});

test('extract-numbers-from-text deep link prefills mode and stats', async ({ page }) => {
  await page.goto('/tools/extract-numbers-from-text/?text=totals%2088%2C%2092%2C%20100&mode=integers&stats=true');
  await expect(page.locator('#in-mode')).toHaveValue('integers', { timeout: 15000 });
  await expect(page.locator('#in-stats')).toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('88', { timeout: 15000 });
  await expect(out).toContainText('92');
  await expect(out).toContainText('100');
  await expect(out).toContainText('Count: 3');
  await expect(out).toContainText('Sum: 280');
});
