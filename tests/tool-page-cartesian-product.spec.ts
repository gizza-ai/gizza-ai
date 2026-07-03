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

test('cartesian-product page runs the exact deep-link example it advertises', async ({ page }) => {
  // The query string below is VERBATIM the "Open it by URL" example the page
  // generates (index.html/index.md) — if this fails, the page teaches users a
  // broken link.
  await page.goto(
    '/tools/cartesian-product/?list1=red%2C%20blue%2C%20green&list2=S%2C%20M%2C%20L&list3=cotton%2C%20linen&list4=slim%2C%20regular&item_separator=auto&dedupe=true&output_format=lines&join_separator=space&custom_join_separator=%20%3A%3A%20&prefix=sku-&suffix=-2026&max_combinations=10000'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('sku-green L linen regular-2026', { timeout: 15000 });
  const lines = (await out.textContent())!.split('\n');
  expect(lines.length).toBe(36); // 3 x 3 x 2 x 2
  expect(lines[0]).toBe('sku-red S cotton slim-2026');
  expect(lines[35]).toBe('sku-green L linen regular-2026');
});

test('cartesian-product page splits a pasted spreadsheet block on tabs and newlines', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  // A 2-D copy from a spreadsheet: tabs between columns, newlines between rows.
  await page.fill('#in-list1', 'red\tblue\ngreen\tteal');
  await page.fill('#in-list2', 'S');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('teal S', { timeout: 15000 });
  expect(await out.textContent()).toBe('red S\nblue S\ngreen S\nteal S');
  // The select advertises the affordance with a friendly label.
  await expect(page.locator('#in-item_separator option[value="tab"]')).toHaveText(
    'Tab — spreadsheet cells'
  );
});

test('cartesian-product page count-only format reports the multiplication, cap-exempt', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  // 150 x 100 = 15000 > default cap 10000 — generation errors, counting works.
  const l150 = Array.from({ length: 150 }, (_, i) => `a${i}`).join(', ');
  const l100 = Array.from({ length: 100 }, (_, i) => `b${i}`).join(', ');
  await page.fill('#in-list1', l150);
  await page.fill('#in-list2', l100);
  await page.selectOption('#in-output_format', 'count');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('150 x 100 = 15000', { timeout: 15000 });
  expect(await out.textContent()).toBe('150 x 100 = 15000');
});

test('cartesian-product page count example chip fills the form and counts', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  await page.getByRole('button', { name: 'Count first — is it too big?' }).click();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 x 3 x 2 = 12', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('count');
});

test('cartesian-product page joins with newline and honours the cap boundary exactly', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  await page.fill('#in-list1', 'a, b');
  await page.fill('#in-list2', '1');
  await page.selectOption('#in-join_separator', 'newline');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('b', { timeout: 15000 });
  expect(await out.textContent()).toBe('a\n1\nb\n1');
  // Cap boundary, both sides: 2 x 1 = 2 combinations; cap 2 generates,
  // cap 1 errors with the exact count.
  await page.fill('#in-max_combinations', '2');
  await expect(out).toContainText('a\n1\nb\n1');
  await page.fill('#in-max_combinations', '1');
  await expect(out).toContainText('would produce 2 combinations', { timeout: 15000 });
  await expect(out).toContainText('max_combinations=1');
});

test('cartesian-product page keeps digits-only and whitespace-edge items verbatim', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  // Digits-only items must not be numerically mangled (007 → 7) on the page
  // surface, and blank/whitespace-only entries are dropped.
  await page.fill('#in-list1', '007, , 08');
  await page.fill('#in-list2', '1.50');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('08 1.50', { timeout: 15000 });
  expect(await out.textContent()).toBe('007 1.50\n08 1.50');
});

test('cartesian-product page download link serves exactly the visible output', async ({ page }) => {
  await page.goto('/tools/cartesian-product/');
  const dl = page.locator('#tool-output-download');
  await expect(dl).toBeHidden(); // nothing to download yet
  await page.fill('#in-list1', 'red, blue');
  await page.fill('#in-list2', 'S, M, L');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('blue L', { timeout: 15000 });
  await expect(dl).toBeVisible();
  expect(await dl.getAttribute('download')).toBe('cartesian-product-output.txt');
  const blobText = await page.evaluate(async () => {
    const a = document.getElementById('tool-output-download') as HTMLAnchorElement;
    return (await fetch(a.href)).text();
  });
  expect(blobText).toBe(COLORS_X_SIZES);
  // An error state hides the download again (no stale export).
  await page.fill('#in-list2', '   ');
  await expect(out).toContainText('list2 has no items', { timeout: 15000 });
  await expect(dl).toBeHidden();
});
