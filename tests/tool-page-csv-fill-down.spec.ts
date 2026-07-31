import { test, expect } from './fixtures';

// Multi-line output: toHaveText normalizes whitespace, so assert textContent exactly.
test('csv-fill-down page fills empty cells down across all columns', async ({ page }) => {
  await page.goto('/tools/csv-fill-down/');
  await page.fill('#in-data', 'region,rep\nWest,Ann\n,\n,Bob\nEast,');
  await expect(page.locator('#tool-output')).toBeVisible({ timeout: 15000 });
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('region,rep\nWest,Ann\nWest,Ann\nWest,Bob\nEast,Bob\n');
});

test('csv-fill-down deep-link: fill up on one column', async ({ page }) => {
  const data = 'region,x\nWest,1\n,2\nEast,3';
  await page.goto(
    '/tools/csv-fill-down/?data=' +
      encodeURIComponent(data) +
      '&columns=region&direction=up',
  );
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('region,x\nWest,1\nEast,2\nEast,3\n');
});
