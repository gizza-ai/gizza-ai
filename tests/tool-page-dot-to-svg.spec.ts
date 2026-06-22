import { test, expect } from './fixtures';

// /tools/dot-to-svg/ renders Graphviz DOT source into an SVG in-browser (pure wasm).
test('dot-to-svg renders a digraph into SVG markup', async ({ page }) => {
  await page.goto('/tools/dot-to-svg/');
  await page.fill('#in-dot', 'digraph { a -> b; b -> c; a -> c; }');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('</svg>');
});

test('dot-to-svg renders an undirected graph', async ({ page }) => {
  await page.goto('/tools/dot-to-svg/');
  await page.fill('#in-dot', 'graph { x -- y; y -- z; }');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
});

test('dot-to-svg dark mode recolors strokes/text', async ({ page }) => {
  await page.goto('/tools/dot-to-svg/');
  await page.check('#in-dark_mode');
  await page.fill('#in-dot', 'digraph { a -> b; }');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('#e6e6e6', { timeout: 15000 });
});
