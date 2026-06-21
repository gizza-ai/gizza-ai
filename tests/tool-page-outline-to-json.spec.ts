import { test, expect } from './fixtures';

test('outline-to-json children format (default), compact', async ({ page }) => {
  await page.goto('/tools/outline-to-json/');

  await page.fill('#in-outline', 'a\n  b\n  c');
  // Default format=children, default pretty (checked) -> uncheck for compact.
  await page.uncheck('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText(
    '[{"text":"a","children":[{"text":"b","children":[]},{"text":"c","children":[]}]}]',
    { timeout: 15000 },
  );
});

test('outline-to-json nested format keys by text', async ({ page }) => {
  await page.goto('/tools/outline-to-json/');

  await page.fill('#in-outline', 'a\n  b\n    c');
  await page.selectOption('#in-format', 'nested');
  await page.uncheck('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText('{"a":{"b":{"c":{}}}}', {
    timeout: 15000,
  });
});

test('outline-to-json pretty-prints by default', async ({ page }) => {
  await page.goto('/tools/outline-to-json/');

  await page.fill('#in-outline', 'root\n  child');
  // Pretty checkbox is checked by default -> multi-line output.
  await expect(page.locator('#tool-output')).toContainText('"text": "root"', {
    timeout: 15000,
  });
});
