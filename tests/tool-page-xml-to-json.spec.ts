import { test, expect } from './fixtures';

// /tools/xml-to-json/ converts XML into an equivalent JSON structure (pure wasm).
test('xml-to-json preserves attributes, nesting, and arrays', async ({ page }) => {
  await page.goto('/tools/xml-to-json/');
  await page.fill(
    '#in-xml',
    '<catalog><book id="1"><title>Dune</title></book><book id="2"><title>Hyperion</title></book></catalog>'
  );
  const out = page.locator('#tool-output');
  // attributes on by default -> @id; repeated <book> collapses to an array.
  await expect(out).toContainText('"@id": "1"', { timeout: 15000 });
  await expect(out).toContainText('"book": [');
  await expect(out).toContainText('"title": "Dune"');
  await expect(out).toContainText('"title": "Hyperion"');
});

test('xml-to-json drops attributes and coerces types when toggled', async ({ page }) => {
  await page.goto('/tools/xml-to-json/');
  await page.fill('#in-xml', '<r id="x"><n>42</n><b>true</b></r>');
  await page.uncheck('#in-attributes');
  await page.check('#in-coerce_types');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"n": 42', { timeout: 15000 });
  await expect(out).toContainText('"b": true');
  // attribute dropped -> no @id key.
  await expect(out).not.toContainText('@id');
});

test('xml-to-json pre-fills from query params', async ({ page }) => {
  await page.goto(
    '/tools/xml-to-json/?xml=' +
      encodeURIComponent('<note><to>World</to></note>') +
      '&attribute_prefix=$'
  );
  await expect(page.locator('#in-attribute_prefix')).toHaveValue('$');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"to": "World"', { timeout: 15000 });
});
