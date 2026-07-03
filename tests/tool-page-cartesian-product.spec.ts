import { test, expect } from './fixtures';

// Exact multi-line outputs (also asserted verbatim in core unit tests + CLI).
const COLORS_X_SIZES =
  'red S\n' +
  'red M\n' +
  'red L\n' +
  'blue S\n' +
  'blue M\n' +
  'blue L';

const SKU_DASH_PREFIX =
  'sku-tee-black\n' +
  'sku-tee-white\n' +
  'sku-hoodie-black\n' +
  'sku-hoodie-white';

const VARIANTS_CSV =
  'red,S,cotton\n' +
  'red,S,linen\n' +
  'red,M,cotton\n' +
  'red,M,linen\n' +
  'blue,S,cotton\n' +
  'blue,S,linen\n' +
  'blue,M,cotton\n' +
  'blue,M,linen';

test('cartesian-product page combines two lists in odometer order exactly', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  await page.fill('#in-list1', 'red, blue');
  await page.fill('#in-list2', 'S, M, L');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('blue L', { timeout: 15000 });
  expect(await out.textContent()).toBe(COLORS_X_SIZES);
});

test('cartesian-product page joins with dash and applies a prefix', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  await page.fill('#in-list1', 'tee, hoodie');
  await page.fill('#in-list2', 'black, white');
  await page.selectOption('#in-join_separator', 'dash');
  await page.fill('#in-prefix', 'sku-');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('sku-hoodie-white', { timeout: 15000 });
  expect(await out.textContent()).toBe(SKU_DASH_PREFIX);
});

test('cartesian-product page emits a three-list product as CSV rows', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  await page.fill('#in-list1', 'red, blue');
  await page.fill('#in-list2', 'S, M');
  await page.fill('#in-list3', 'cotton, linen');
  await page.selectOption('#in-output_format', 'csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('blue,M,linen', { timeout: 15000 });
  expect(await out.textContent()).toBe(VARIANTS_CSV);
});

test('cartesian-product page errors with the exact count when the cap is exceeded', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  const l150 = Array.from({ length: 150 }, (_, i) => `a${i}`).join(', ');
  const l100 = Array.from({ length: 100 }, (_, i) => `b${i}`).join(', ');
  await page.fill('#in-list1', l150);
  await page.fill('#in-list2', l100);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('would produce 15000 combinations', { timeout: 15000 });
  await expect(out).toContainText('max_combinations=10000');
});

test('cartesian-product page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/cartesian-product/?list1=red%2C%20blue&list2=S%2C%20M%2C%20L');
  await expect(page.locator('#in-list1')).toHaveValue('red, blue');
  await expect(page.locator('#in-list2')).toHaveValue('S, M, L');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('blue L', { timeout: 15000 });
  expect(await out.textContent()).toBe(COLORS_X_SIZES);
});
