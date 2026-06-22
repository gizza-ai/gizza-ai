import { test, expect } from './fixtures';

// /tools/color-format-convert/ converts colors in-browser (pure wasm).
test('color converter shows all formats for a hex color', async ({ page }) => {
  await page.goto('/tools/color-format-convert/?color=' + encodeURIComponent('#3498db'));
  const out = page.locator('#tool-output');
  await expect(out).toContainText('rgb(52, 152, 219)', { timeout: 15000 });
  await expect(out).toContainText('hsl(204, 70%, 53%)');
  await expect(out).toContainText('cmyk(76%, 31%, 0%, 14%)');
});

test('color converter errors on an invalid color', async ({ page }) => {
  await page.goto('/tools/color-format-convert/');
  await page.fill('#in-color', 'notacolor');
  await expect(page.locator('#tool-output')).toContainText('unrecognized', { timeout: 15000 });
});
