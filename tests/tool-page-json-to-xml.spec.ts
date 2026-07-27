import { test, expect } from './fixtures';

const sample = '{"book":{"@id":"dune","title":"Dune","authors":["Frank Herbert"]}}';

test('json-to-xml page renders exact pretty XML with attributes and arrays', async ({ page }) => {
  await page.goto('/tools/json-to-xml/');
  await page.fill('#in-json', sample);
  await page.fill('#in-root_element', 'catalog');
  await page.fill('#in-array_item_element', 'author');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<catalog>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<catalog>\n  <book id="dune">\n    <title>Dune</title>\n    <authors>\n      <author>Frank Herbert</author>\n    </authors>\n  </book>\n</catalog>');
});

test('json-to-xml deep link covers compact format and XML declaration checkbox', async ({ page }) => {
  const qs =
    '?json=' + encodeURIComponent('{"ok":true,"count":2}') +
    '&format=compact' +
    '&xml_declaration=true' +
    '&root_element=response';
  await page.goto('/tools/json-to-xml/' + qs);

  await expect(page.locator('#in-format')).toHaveValue('compact', { timeout: 15_000 });
  await expect(page.locator('#in-xml_declaration')).toBeChecked();
  await expect(page.locator('#in-root_element')).toHaveValue('response');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<?xml version="1.0" encoding="UTF-8"?>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<?xml version="1.0" encoding="UTF-8"?><response><ok>true</ok><count>2</count></response>');
});

test('json-to-xml page can disable attributes and change text key', async ({ page }) => {
  await page.goto('/tools/json-to-xml/');
  await page.fill('#in-json', '{"p":{"@id":"x","text":"hello"}}');
  await page.fill('#in-attribute_prefix', '');
  await page.fill('#in-text_key', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<_id>x</_id>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<root>\n  <p>\n    hello\n    <_id>x</_id>\n  </p>\n</root>');
});

test('json-to-xml page reports invalid JSON clearly', async ({ page }) => {
  await page.goto('/tools/json-to-xml/');
  await page.fill('#in-json', '{bad');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15_000 });
});
