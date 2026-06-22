import { test, expect } from './fixtures';

// /tools/count-line-frequency/ tallies line frequencies in-browser (pure wasm).
test('count-line-frequency ranks lines by occurrence', async ({ page }) => {
  await page.goto('/tools/count-line-frequency/');
  await page.fill('#in-text', 'apple\nbanana\napple\ncherry\napple\nbanana');
  const out = page.locator('#tool-output');
  // most-frequent first: "3<TAB>apple" then "2<TAB>banana".
  await expect(out).toContainText('3\tapple', { timeout: 15000 });
  await expect(out).toContainText('2\tbanana');
  await expect(out).toContainText('1\tcherry');
});
