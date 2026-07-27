import { test, expect } from './fixtures';

const HELLO_BASE64 = 'FgAAAAJoZWxsbwAGAAAAd29ybGQAAA==';
const TYPED_HEX = '1e000000106900070000 00126c000900000000000000086200010a6e0000';

test('bson-inspector decodes base64 BSON into a typed tree', async ({ page }) => {
  await page.goto('/tools/bson-inspector/');
  await page.fill('#in-input', HELLO_BASE64);

  await expect(page.locator('#tool-output')).toHaveText('hello: String "world"', {
    timeout: 15000,
  });
});

test('bson-inspector converts hex BSON to canonical Extended JSON', async ({ page }) => {
  await page.goto('/tools/bson-inspector/');
  await page.fill('#in-input', HELLO_BASE64);
  await page.selectOption('#in-output', 'json');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText('{"hello":"world"}', {
    timeout: 15000,
  });
});

test('bson-inspector deep-link honors hex input, tree output, and offsets', async ({ page }) => {
  const params = new URLSearchParams({
    input: TYPED_HEX,
    input_format: 'hex',
    output: 'tree',
    indent: '2',
    show_offsets: 'true',
  });

  await page.goto(`/tools/bson-inspector/?${params.toString()}`);
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-show_offsets')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('@4      i: Int32 7', { timeout: 15000 });
  await expect(out).toContainText('l: Int64 9');
  await expect(out).toContainText('b: Boolean true');
  await expect(out).toContainText('n: Null');
});
