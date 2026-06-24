import { test, expect } from './fixtures';

test('color-contrast-checker page — black on white passes AAA', async ({ page }) => {
  await page.goto('/tools/color-contrast-checker/');

  await page.fill('#in-foreground', '#000000');
  await page.fill('#in-background', '#ffffff');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Contrast ratio: 21:1', { timeout: 15000 });
  await expect(out).toContainText('Best level for normal text: AAA');
});

test('color-contrast-checker page — rgb input + json format', async ({ page }) => {
  await page.goto('/tools/color-contrast-checker/');

  await page.fill('#in-foreground', 'rgb(118,118,118)');
  await page.fill('#in-background', '#ffffff');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"ratio":4.54', { timeout: 15000 });
  await expect(out).toContainText('"summary":"AA"');
});

test('color-contrast-checker page — named colour + suggest mode', async ({ page }) => {
  await page.goto('/tools/color-contrast-checker/');

  // Named CSS colour background + the suggest format with an AA target.
  await page.fill('#in-foreground', '#aaaaaa');
  await page.fill('#in-background', 'white');
  await page.selectOption('#in-format', 'suggest');
  await page.selectOption('#in-target', 'aa');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Suggested foreground for AA', { timeout: 15000 });
  await expect(out).toContainText('#767676');
});

test('color-contrast-checker page — query params deep-link', async ({ page }) => {
  await page.goto('/tools/color-contrast-checker/?foreground=%23aaaaaa&background=%23ffffff');

  await expect(page.locator('#in-foreground')).toHaveValue('#aaaaaa');
  await expect(page.locator('#in-background')).toHaveValue('#ffffff');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Contrast ratio: 2.32:1', { timeout: 15000 });
  await expect(out).toContainText('Best level for normal text: Fail');
});
