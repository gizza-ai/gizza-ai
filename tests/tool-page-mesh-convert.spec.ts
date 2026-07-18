import { test, expect } from './fixtures';

const triObj = 'v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n';
const triStl = 'solid tri\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid tri\n';

test('mesh-convert converts OBJ triangle to ASCII STL', async ({ page }) => {
  await page.goto('/tools/mesh-convert/');
  await page.fill('#in-mesh', triObj);
  await page.fill('#in-name', 'triangle');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('solid triangle', { timeout: 15000 });
  await expect(out).toContainText('facet normal 0 0 1');
  await expect(out).toContainText('vertex 1 0 0');
  await expect(out).toContainText('endsolid triangle');
});

test('mesh-convert converts ASCII STL back to OBJ', async ({ page }) => {
  await page.goto('/tools/mesh-convert/');
  await page.selectOption('#in-to', 'obj');
  await page.fill('#in-mesh', triStl);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('o mesh', { timeout: 15000 });
  await expect(out).toContainText('v 0 0 0');
  await expect(out).toContainText('f 1 2 3');
});

test('mesh-convert supports query-param deep link', async ({ page }) => {
  const mesh = encodeURIComponent(triObj.trim());
  await page.goto(`/tools/mesh-convert/?mesh=${mesh}&to=stl&stl_encoding=ascii&scale=2&axis=keep&name=scaled`);
  await expect(page.locator('#in-mesh')).toHaveValue(triObj.trim());
  await expect(page.locator('#in-scale')).toHaveValue('2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('solid scaled', { timeout: 15000 });
  await expect(out).toContainText('vertex 2 0 0');
});
