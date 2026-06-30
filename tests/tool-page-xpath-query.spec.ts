import { test, expect } from './fixtures';

const xml = '<catalog><book id="b1"><title>Rust</title><price>30</price></book><book id="b2"><title>XML</title><price>9</price></book></catalog>';

test('xpath-query page extracts title values', async ({ page }) => {
  await page.goto('/tools/xpath-query/');
  await page.fill('#in-expression', '//book/title');
  await page.fill('#in-xml', xml);
  await page.selectOption('#in-output', 'value');
  await expect(page.locator('#tool-output')).toHaveText('Rust\nXML', { timeout: 15000 });
});

test('xpath-query page evaluates scalar count', async ({ page }) => {
  await page.goto('/tools/xpath-query/');
  await page.fill('#in-expression', 'count(//book)');
  await page.fill('#in-xml', xml);
  await expect(page.locator('#tool-output')).toHaveText('2', { timeout: 15000 });
});

test('xpath-query page serializes matching XML nodes', async ({ page }) => {
  await page.goto('/tools/xpath-query/');
  await page.fill('#in-expression', "//book[@id='b2']");
  await page.fill('#in-xml', xml);
  await page.selectOption('#in-output', 'xml');
  await expect(page.locator('#tool-output')).toHaveText('<book id="b2"><title>XML</title><price>9</price></book>', { timeout: 15000 });
});

test('xpath-query page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/xpath-query/?expression=' + encodeURIComponent('count(//book)') + '&xml=' + encodeURIComponent(xml) + '&output=value');
  await expect(page.locator('#in-expression')).toHaveValue('count(//book)');
  await expect(page.locator('#tool-output')).toHaveText('2', { timeout: 15000 });
});
