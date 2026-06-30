import { test, expect } from './fixtures';

// /tools/slugify/ turns a title into a URL-safe slug in-browser (pure wasm, live).
test('slugify makes a basic slug from a title', async ({ page }) => {
  await page.goto('/tools/slugify/');
  await page.fill('#in-text', '10 Tips for Crème Brûlée!');
  await expect(page.locator('#tool-output')).toHaveText('10-tips-for-creme-brulee', {
    timeout: 15000,
  });
});

test('slugify underscore separator and preserved case', async ({ page }) => {
  await page.goto('/tools/slugify/');
  await page.fill('#in-text', 'Hello World Test');
  await page.fill('#in-separator', '_');
  await page.uncheck('#in-lowercase');
  await expect(page.locator('#tool-output')).toHaveText('Hello_World_Test', {
    timeout: 15000,
  });
});

test('slugify max length truncates on a word boundary', async ({ page }) => {
  await page.goto('/tools/slugify/');
  await page.fill('#in-text', 'The Quick Brown Fox');
  await page.fill('#in-max_length', '13');
  await expect(page.locator('#tool-output')).toHaveText('the-quick', {
    timeout: 15000,
  });
});

test('slugify per-line slugifies a batch of titles', async ({ page }) => {
  await page.goto('/tools/slugify/');
  await page.fill('#in-text', 'Hello World\nFoo & Bar');
  await page.check('#in-per_line');
  await expect(page.locator('#tool-output')).toHaveText('hello-world\nfoo-bar', {
    timeout: 15000,
  });
});

test('slugify query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/slugify/?text=Bob%27s%20Burgers');
  await expect(page.locator('#in-text')).toHaveValue("Bob's Burgers", {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText('bobs-burgers', {
    timeout: 15000,
  });
});
