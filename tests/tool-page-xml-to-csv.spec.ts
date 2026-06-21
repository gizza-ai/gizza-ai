import { test, expect } from './fixtures';

// /tools/xml-to-csv/ flattens repeated XML elements into a CSV table (pure wasm).
test('xml-to-csv auto-detects records and includes attributes', async ({ page }) => {
  await page.goto('/tools/xml-to-csv/');
  await page.fill(
    '#in-xml',
    '<catalog><book id="1"><title>Dune</title><author>Herbert</author></book><book id="2"><title>Hyperion</title><author>Simmons</author></book></catalog>'
  );
  // attributes checkbox is checked by default; record left blank -> auto-detect.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('@id,title,author', { timeout: 15000 });
  await expect(out).toContainText('1,Dune,Herbert');
  await expect(out).toContainText('2,Hyperion,Simmons');
});

test('xml-to-csv drops attributes when unchecked, honors record + tab', async ({ page }) => {
  await page.goto('/tools/xml-to-csv/');
  await page.fill('#in-xml', '<rows><row><a>1</a><b>2</b></row></rows>');
  await page.fill('#in-record', 'row');
  await page.uncheck('#in-attributes');
  await page.fill('#in-delimiter', 'tab');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a\tb', { timeout: 15000 });
  await expect(out).toContainText('1\t2');
});

test('xml-to-csv pre-fills from query params', async ({ page }) => {
  await page.goto(
    '/tools/xml-to-csv/?xml=' +
      encodeURIComponent('<data><item><n>A</n></item><item><n>B</n></item></data>') +
      '&record=item&attributes=false'
  );
  await expect(page.locator('#in-record')).toHaveValue('item');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('n', { timeout: 15000 });
  await expect(out).toContainText('A');
  await expect(out).toContainText('B');
});
