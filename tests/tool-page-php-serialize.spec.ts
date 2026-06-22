import { test, expect } from './fixtures';

test('php-serialize page', async ({ page }) => {
  await page.goto('/tools/php-serialize/');
  await page.fill('#in-json', '{"name": "Al", "age": 30}');
  await expect(page.locator('#tool-output')).toHaveText(
    'a:2:{s:4:"name";s:2:"Al";s:3:"age";i:30;}',
    { timeout: 15000 }
  );
});
