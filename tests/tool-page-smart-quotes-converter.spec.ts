import { test, expect } from './fixtures';

test('smart-quotes-converter educates straight quotes into curly by default', async ({ page }) => {
  await page.goto('/tools/smart-quotes-converter/');
  await page.fill('#in-input', '"Hello," she said. It\'s a \'test\'.');
  await expect(page.locator('#tool-output')).toHaveText(
    '“Hello,” she said. It’s a ‘test’.',
    { timeout: 15000 },
  );
});

test('smart-quotes-converter honours a feet-and-inches query-param deep link', async ({ page }) => {
  await page.goto('/tools/smart-quotes-converter/?input=6\'4%22&direction=to_curly&feet_inches=true');
  await expect(page.locator('#in-input')).toHaveValue('6\'4"');
  await expect(page.locator('#in-direction')).toHaveValue('to_curly');
  await expect(page.locator('#in-feet_inches')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('6′4″', { timeout: 15000 });
});

test('smart-quotes-converter leaves apostrophes straight when convert_single is unchecked', async ({ page }) => {
  await page.goto('/tools/smart-quotes-converter/');
  await page.fill('#in-input', 'It\'s "quoted"');
  await page.uncheck('#in-convert_single');
  await expect(page.locator('#tool-output')).toHaveText(
    'It\'s “quoted”',
    { timeout: 15000 },
  );
});
