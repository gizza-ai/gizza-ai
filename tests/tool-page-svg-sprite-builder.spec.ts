import { test, expect } from './fixtures';

test('svg-sprite-builder page builds a symbol sprite', async ({ page }) => {
  await page.goto('/tools/svg-sprite-builder/');
  await page.fill(
    '#in-svgs',
    '<svg viewBox="0 0 24 24"><path d="M1 1"/></svg><svg viewBox="0 0 16 16"><circle r="4"/></svg>'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<symbol id="icon-1" viewBox="0 0 24 24">', { timeout: 15000 });
  await expect(out).toContainText('<symbol id="icon-2" viewBox="0 0 16 16">');
  await expect(out).toContainText('<use href="#icon-1" />');
});

test('svg-sprite-builder page honors id_source=title and prefix', async ({ page }) => {
  await page.goto('/tools/svg-sprite-builder/');
  await page.fill('#in-svgs', '<svg viewBox="0 0 1 1"><title>Arrow Up</title><path/></svg>');
  await page.fill('#in-prefix', 'ic');
  await page.selectOption('#in-id_source', 'title');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('id="arrow-up"', { timeout: 15000 });
});
