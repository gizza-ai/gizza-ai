import { test, expect } from './fixtures';

test('amcache-parser page rejects truncated hive bytes exactly', async ({ page }) => {
  await page.goto('/tools/amcache-parser/?data=72656766&input_encoding=hex&mode=report');
  await expect(page.locator('#tool-output')).toContainText('input is only 4 byte(s)', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('4096-byte base block');
});

test('amcache-parser page supports enum controls and deep-linked csv mode', async ({ page }) => {
  await page.goto('/tools/amcache-parser/?data=72656766&input_encoding=hex&section=files&mode=csv&association=all&sort=path&max_entries=25');
  await expect(page.locator('#tool-output')).toContainText('input is only 4 byte(s)', { timeout: 15000 });
});
