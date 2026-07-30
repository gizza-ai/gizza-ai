import { test, expect } from './fixtures';

// /tools/html-entity-encoder/ encodes literal characters into HTML character
// references entirely in-browser (pure wasm). Field ids are in-<name>:
// #in-text is multiline, #in-scope and #in-format are descriptor-backed selects.

test('html-entity-encoder page: minimal named encodes the five HTML-sensitive characters', async ({ page }) => {
  await page.goto('/tools/html-entity-encoder/');
  await page.fill('#in-text', '<a href="x">Tom & Jerry\'s</a>');
  await expect(page.locator('#tool-output')).toHaveText(
    '&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&apos;s&lt;/a&gt;',
    { timeout: 15000 },
  );
});

test('html-entity-encoder page: non-ascii named uses names and numeric fallback', async ({ page }) => {
  await page.goto('/tools/html-entity-encoder/');
  await page.fill('#in-text', 'A © — 😀 &');
  await page.selectOption('#in-scope', 'non-ascii');
  await page.selectOption('#in-format', 'named');
  await expect(page.locator('#tool-output')).toHaveText(
    'A &copy; &mdash; &#128512; &amp;',
    { timeout: 15000 },
  );
});

test('html-entity-encoder page: named scope encodes named characters beyond non-ascii', async ({ page }) => {
  await page.goto('/tools/html-entity-encoder/');
  await page.fill('#in-text', 'é and z');
  await page.selectOption('#in-scope', 'named');
  await page.selectOption('#in-format', 'named');
  await expect(page.locator('#tool-output')).toHaveText('&eacute; and z', {
    timeout: 15000,
  });
});

test('html-entity-encoder page: decimal format is always numeric', async ({ page }) => {
  await page.goto('/tools/html-entity-encoder/');
  await page.fill('#in-text', '© & <');
  await page.selectOption('#in-scope', 'non-ascii');
  await page.selectOption('#in-format', 'decimal');
  await expect(page.locator('#tool-output')).toHaveText('&#169; &#38; &#60;', {
    timeout: 15000,
  });
});

test('html-entity-encoder page: hex format is always numeric hex', async ({ page }) => {
  await page.goto('/tools/html-entity-encoder/');
  await page.fill('#in-text', '© & <');
  await page.selectOption('#in-scope', 'non-ascii');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('&#xA9; &#x26; &#x3C;', {
    timeout: 15000,
  });
});

test('html-entity-encoder page: query-param deep-link prefills and encodes', async ({ page }) => {
  await page.goto(
    '/tools/html-entity-encoder/?text=' +
      encodeURIComponent('© & <') +
      '&scope=non-ascii&format=hex',
  );
  await expect(page.locator('#in-text')).toHaveValue('© & <', { timeout: 15000 });
  await expect(page.locator('#in-scope')).toHaveValue('non-ascii');
  await expect(page.locator('#in-format')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toHaveText('&#xA9; &#x26; &#x3C;', {
    timeout: 15000,
  });
});
