import { test, expect } from './fixtures';

// /tools/svg-optimize/ minifies + cleans up SVG in-browser (pure wasm).
test('svg-optimize collapses whitespace, strips comments + metadata', async ({ page }) => {
  await page.goto('/tools/svg-optimize/');
  // "Remove comments" + "Remove editor metadata" are checked by default.
  await page.fill(
    '#in-svg',
    '<?xml version="1.0"?>\n<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">\n  <!-- icon -->\n  <metadata>junk</metadata>\n  <rect x="0" y="0"/>\n</svg>',
  );
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="0" y="0"/></svg>',
    { timeout: 15000 },
  );
});

test('svg-optimize keeps comments when the box is unchecked', async ({ page }) => {
  await page.goto('/tools/svg-optimize/');
  await page.uncheck('#in-remove_comments');
  await page.fill('#in-svg', '<svg xmlns="http://www.w3.org/2000/svg"><!-- keep --><rect/></svg>');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<!-- keep -->', { timeout: 15000 });
});

test('svg-optimize removes id/class and root dimensions when toggled', async ({ page }) => {
  await page.goto('/tools/svg-optimize/');
  await page.check('#in-remove_ids');
  await page.check('#in-remove_dimensions');
  await page.fill(
    '#in-svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect id="r" class="c" x="0"/></svg>',
  );
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="0"/></svg>',
    { timeout: 15000 },
  );
});
