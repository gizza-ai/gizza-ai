import { test, expect } from './fixtures';

// /tools/color-shades-generator/ builds tints/shades/tones ramps in-browser (pure wasm).
test('shades generator returns a Tailwind-style 50-900 scale', async ({ page }) => {
  await page.goto('/tools/color-shades-generator/');
  await page.fill('#in-color', '#3498db');
  await page.selectOption('#in-mode', 'scale');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('scale ramp from #3498db', { timeout: 15000 });
  // all eleven Tailwind weights present (50-950), base at 500.
  await expect(out).toContainText('50:');
  await expect(out).toContainText('500: #3498db  (base)');
  await expect(out).toContainText('950:');
});

test('shades generator respects the steps count for tints', async ({ page }) => {
  await page.goto('/tools/color-shades-generator/');
  await page.fill('#in-color', '#ff5733');
  await page.selectOption('#in-mode', 'tints');
  await page.fill('#in-count', '5');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('tints ramp from #ff5733', { timeout: 15000 });
  await expect(out).toContainText('#ff5733');
  // last tint reaches white.
  await expect(out).toContainText('5: #ffffff');
});

test('shades generator errors on an invalid color', async ({ page }) => {
  await page.goto('/tools/color-shades-generator/');
  await page.fill('#in-color', 'notacolor');
  await expect(page.locator('#tool-output')).toContainText('unrecognized', { timeout: 15000 });
});
