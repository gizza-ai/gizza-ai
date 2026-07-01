import { test, expect } from './fixtures';

test('add-line-numbers numbers every line with defaults', async ({ page }) => {
  await page.goto('/tools/add-line-numbers/');
  await page.fill('#in-text', 'alpha\nbeta\ngamma');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1\talpha', { timeout: 15000 });
  await expect(out).toContainText('2\tbeta');
  await expect(out).toContainText('3\tgamma');
});

test('add-line-numbers supports start, step, separator, and zero padding', async ({ page }) => {
  await page.goto('/tools/add-line-numbers/');
  await page.fill('#in-text', 'alpha\nbeta\ngamma');
  await page.fill('#in-start', '10');
  await page.fill('#in-step', '5');
  await page.fill('#in-separator', ': ');
  await page.selectOption('#in-pad', 'zeros');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('10: alpha', { timeout: 15000 });
  await expect(out).toContainText('15: beta');
  await expect(out).toContainText('20: gamma');
});

test('add-line-numbers can skip blank lines', async ({ page }) => {
  await page.goto('/tools/add-line-numbers/');
  await page.fill('#in-text', 'alpha\n\n  \nbeta');
  await page.fill('#in-separator', '. ');
  await page.selectOption('#in-pad', 'none');
  await page.check('#in-number_nonblank');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1. alpha', { timeout: 15000 });
  await expect(out).toContainText('2. beta');
});
