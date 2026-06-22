import { test, expect } from './fixtures';
test('csv-query page select+where', async ({ page }) => {
  await page.goto('/tools/csv-query/');
  await page.fill('#in-data', 'name,age\nAlice,30\nBob,25');
  await page.fill('#in-q', 'SELECT name WHERE age >= 28');
  await expect(page.locator('#tool-output')).toHaveText('name\nAlice', { timeout: 15000 });
});
