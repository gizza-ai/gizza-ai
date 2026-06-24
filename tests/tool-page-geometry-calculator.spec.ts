import { test, expect } from './fixtures';

test('geometry-calculator page computes circle area and perimeter', async ({ page }) => {
  await page.goto('/tools/geometry-calculator/');

  await page.selectOption('#in-shape', 'circle');
  await page.fill('#in-radius', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"shape": "circle"', { timeout: 15000 });
  await expect(out).toContainText('"dimensionality": "2D"');
  await expect(out).toContainText('"name": "area"');
  await expect(out).toContainText('12.566371');
});

test('geometry-calculator page computes 3D box surface area and volume', async ({ page }) => {
  await page.goto('/tools/geometry-calculator/');

  await page.selectOption('#in-shape', 'rectangular_prism');
  await page.fill('#in-width', '2');
  await page.fill('#in-height', '3');
  await page.fill('#in-length', '4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"dimensionality": "3D"', { timeout: 15000 });
  await expect(out).toContainText('"name": "surface_area"');
  await expect(out).toContainText('"value": 52.0');
  await expect(out).toContainText('"name": "volume"');
  await expect(out).toContainText('"value": 24.0');
});

test('geometry-calculator page deep-links shape + dimensions via query params', async ({ page }) => {
  await page.goto('/tools/geometry-calculator/?shape=sphere&radius=3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"shape": "sphere"', { timeout: 15000 });
  await expect(out).toContainText('113.097336');
});

test('geometry-calculator page shows a prompt before any dimension is entered', async ({ page }) => {
  await page.goto('/tools/geometry-calculator/');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Enter the dimensions', { timeout: 15000 });
});
