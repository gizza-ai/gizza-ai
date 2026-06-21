import { test, expect } from './fixtures';

// /tools/color-palette-generator/ builds a harmonious palette in-browser (pure wasm).
test('palette generator returns triadic colors for a hex base', async ({ page }) => {
  await page.goto('/tools/color-palette-generator/');
  await page.fill('#in-color', '#ff0000');
  await page.selectOption('#in-scheme', 'triadic');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('triadic palette', { timeout: 15000 });
  await expect(out).toContainText('#ff0000');
  await expect(out).toContainText('#00ff00');
  await expect(out).toContainText('#0000ff');
});

test('palette generator respects the colors count for series schemes', async ({ page }) => {
  await page.goto('/tools/color-palette-generator/');
  await page.fill('#in-color', '#3498db');
  await page.selectOption('#in-scheme', 'analogous');
  await page.fill('#in-count', '5');
  const out = page.locator('#tool-output');
  // base color is the center swatch and numbering reaches 5.
  await expect(out).toContainText('analogous palette', { timeout: 15000 });
  await expect(out).toContainText('5.');
  await expect(out).toContainText('#3498db');
});

test('palette generator errors on an invalid color', async ({ page }) => {
  await page.goto('/tools/color-palette-generator/');
  await page.fill('#in-color', 'notacolor');
  await expect(page.locator('#tool-output')).toContainText('unrecognized', { timeout: 15000 });
});
