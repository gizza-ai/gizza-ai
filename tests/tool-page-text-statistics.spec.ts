import { test, expect } from './fixtures';

// /tools/text-statistics/ counts text metrics in-browser (pure wasm).
// The text field is a multiline <textarea>.
test('text-statistics page reports word and sentence counts', async ({ page }) => {
  await page.goto('/tools/text-statistics/');
  await page.fill('#in-text', 'Hello world. How are you today?');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Words: 6', { timeout: 15000 });
  await expect(out).toContainText('Sentences: 2');
  await expect(out).toContainText('Reading time:');
});
