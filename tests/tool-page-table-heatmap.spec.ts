import { test, expect } from './fixtures';

// /tools/table-heatmap/ shades a CSV's numeric cells with a color scale and emits
// a styled HTML <table> (pure wasm, in-browser). scale is a <select>; header and
// per_column are checkboxes (default checked); data/delimiter are fields.
test('table-heatmap shades numeric cells and keeps a header', async ({ page }) => {
  await page.goto('/tools/table-heatmap/');
  await page.fill('#in-data', 'region,q1,q2\nNorth,120,90\nSouth,60,150');
  await page.selectOption('#in-scale', 'red-yellow-green');
  const out = page.locator('#tool-output');
  // header cell is plain (grey), numeric cells get an rgb() background
  await expect(out).toContainText('<th style=', { timeout: 15000 });
  await expect(out).toContainText('>region</th>');
  await expect(out).toContainText('background:rgb(');
  // North q1=120 is the per-column max → green anchor
  await expect(out).toContainText('background:rgb(87,171,90)');
});

test('table-heatmap honors the blue scale', async ({ page }) => {
  await page.goto('/tools/table-heatmap/');
  await page.fill('#in-data', 'v\n0\n100');
  await page.selectOption('#in-scale', 'blue');
  // max value gets the blue anchor (40,110,200) on the white→blue scale
  await expect(page.locator('#tool-output')).toContainText('background:rgb(40,110,200)', {
    timeout: 15000,
  });
});

test('table-heatmap applies fixed min/max bounds', async ({ page }) => {
  await page.goto('/tools/table-heatmap/');
  await page.fill('#in-data', 'v\n40\n60');
  await page.selectOption('#in-scale', 'green');
  await page.fill('#in-min', '0');
  await page.fill('#in-max', '100');
  const out = page.locator('#tool-output');
  // with fixed 0..100, 40 is t=0.4 (mid-green), not the column-min white.
  await expect(out).toContainText('background:rgb(', { timeout: 15000 });
  await expect(out).not.toContainText('background:rgb(255,255,255)');
});
