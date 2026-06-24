import { test, expect } from './fixtures';

test('php-unserialize page', async ({ page }) => {
  await page.goto('/tools/php-unserialize/');
  await page.fill('#in-serialized', 'a:2:{s:4:"name";s:2:"Al";s:3:"age";i:30;}');
  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "name": "Al",\n  "age": 30\n}',
    { timeout: 15000 }
  );
});

test('php-unserialize page query-param deep-link', async ({ page }) => {
  await page.goto('/tools/php-unserialize/?serialized=' + encodeURIComponent('a:3:{i:0;i:1;i:1;i:2;i:2;i:3;}'));
  await expect(page.locator('#tool-output')).toHaveText(
    '[\n  1,\n  2,\n  3\n]',
    { timeout: 15000 }
  );
});
