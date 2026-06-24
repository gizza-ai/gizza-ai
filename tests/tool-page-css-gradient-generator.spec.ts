import { test, expect } from './fixtures';

test('css-gradient-generator linear page', async ({ page }) => {
  await page.goto('/tools/css-gradient-generator/');

  // Default type=linear; set colors + angle.
  await page.fill('#in-colors', '#ff0000, #0000ff');
  await page.fill('#in-angle', '90');
  await expect(page.locator('#tool-output')).toHaveText(
    'background-image: linear-gradient(90deg, #ff0000 0%, #0000ff 100%);',
    { timeout: 15000 },
  );
});

test('css-gradient-generator radial circle', async ({ page }) => {
  await page.goto('/tools/css-gradient-generator/');
  await page.fill('#in-colors', '#fff, #000');
  await page.selectOption('#in-gradient_type', 'radial');
  await page.selectOption('#in-shape', 'circle');
  await expect(page.locator('#tool-output')).toHaveText(
    'background-image: radial-gradient(circle at center, #fff 0%, #000 100%);',
    { timeout: 15000 },
  );
});

test('css-gradient-generator interpolation color space', async ({ page }) => {
  await page.goto('/tools/css-gradient-generator/');
  await page.fill('#in-colors', '#f00, #00f');
  await page.fill('#in-angle', '90');
  await page.fill('#in-interpolation', 'oklch');
  await expect(page.locator('#tool-output')).toHaveText(
    'background-image: linear-gradient(in oklch 90deg, #f00 0%, #00f 100%);',
    { timeout: 15000 },
  );
});

test('css-gradient-generator repeating conic', async ({ page }) => {
  await page.goto('/tools/css-gradient-generator/');
  await page.fill('#in-colors', 'red, yellow, red');
  await page.selectOption('#in-gradient_type', 'conic');
  await page.fill('#in-angle', '90');
  await page.check('#in-repeating');
  await expect(page.locator('#tool-output')).toHaveText(
    'background-image: repeating-conic-gradient(from 90deg at center, red 0%, yellow 50%, red 100%);',
    { timeout: 15000 },
  );
});
