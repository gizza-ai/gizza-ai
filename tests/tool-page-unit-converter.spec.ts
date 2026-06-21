import { test, expect } from './fixtures';

test('unit-converter page converts km to miles', async ({ page }) => {
  await page.goto('/tools/unit-converter/');
  await page.fill('#in-value', '5');
  await page.fill('#in-from', 'km');
  await page.fill('#in-to', 'miles');
  await expect(page.locator('#tool-output')).toContainText('3.106855961187 mile', {
    timeout: 15000,
  });
});

test('unit-converter page converts celsius to fahrenheit', async ({ page }) => {
  await page.goto('/tools/unit-converter/');
  await page.fill('#in-value', '100');
  await page.fill('#in-from', 'celsius');
  await page.fill('#in-to', 'fahrenheit');
  await expect(page.locator('#tool-output')).toContainText('212 fahrenheit', {
    timeout: 15000,
  });
});
