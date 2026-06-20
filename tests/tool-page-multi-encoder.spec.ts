import { test, expect } from './fixtures';

// /tools/multi-encoder/ encodes/decodes text in-browser (pure wasm).
test('multi-encoder page base64-encodes text', async ({ page }) => {
  await page.goto('/tools/multi-encoder/');
  await page.fill('#in-text', 'hi');
  await page.selectOption('#in-encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('aGk=', { timeout: 15000 });
});

test('multi-encoder page decodes morse via deep-link', async ({ page }) => {
  const qs = '?text=' + encodeURIComponent('... --- ...') + '&encoding=morse&direction=decode';
  await page.goto('/tools/multi-encoder/' + qs);
  await expect(page.locator('#tool-output')).toContainText('SOS', { timeout: 15000 });
});
