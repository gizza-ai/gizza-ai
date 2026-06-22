import { test, expect } from './fixtures';

test('split-text page splits on comma with trim', async ({ page }) => {
  await page.goto('/tools/split-text/');

  // Default delimiter ',', default mode literal. Turn on trim.
  await page.fill('#in-text', 'apple, banana, cherry');
  await page.check('#in-trim');
  await expect(page.locator('#tool-output')).toHaveText('apple\nbanana\ncherry', {
    timeout: 15000,
  });
});

test('split-text whitespace mode collapses runs', async ({ page }) => {
  await page.goto('/tools/split-text/');
  await page.fill('#in-text', 'the quick   brown fox');
  await page.selectOption('#in-mode', 'whitespace');
  await expect(page.locator('#tool-output')).toHaveText('the\nquick\nbrown\nfox', {
    timeout: 15000,
  });
});

test('split-text chars mode emits one character per line', async ({ page }) => {
  await page.goto('/tools/split-text/');
  await page.fill('#in-text', 'héllo');
  await page.selectOption('#in-mode', 'chars');
  await expect(page.locator('#tool-output')).toHaveText('h\né\nl\nl\no', {
    timeout: 15000,
  });
});

test('split-text remove-empty drops blanks from trailing delimiter', async ({ page }) => {
  await page.goto('/tools/split-text/');
  await page.fill('#in-text', 'a,,b,');
  await page.check('#in-remove_empty');
  await expect(page.locator('#tool-output')).toHaveText('a\nb', { timeout: 15000 });
});

test('split-text tab escape delimiter', async ({ page }) => {
  await page.goto('/tools/split-text/');
  await page.fill('#in-text', 'id\tname\temail');
  await page.fill('#in-delimiter', '\\t');
  await expect(page.locator('#tool-output')).toHaveText('id\nname\nemail', {
    timeout: 15000,
  });
});

test('split-text query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/split-text/?text=' + encodeURIComponent('a;b;c') + '&delimiter=' + encodeURIComponent(';'),
  );
  await expect(page.locator('#in-text')).toHaveValue('a;b;c', { timeout: 15000 });
  await expect(page.locator('#in-delimiter')).toHaveValue(';');
  await expect(page.locator('#tool-output')).toHaveText('a\nb\nc', { timeout: 15000 });
});
