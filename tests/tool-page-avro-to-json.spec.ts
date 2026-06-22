import { test, expect } from './fixtures';

// A real Avro OCF (record User{name:string, age:int, admin:boolean}) with two
// records (Ada/36/true, Linus/54/false), base64-encoded.
const OCF_B64 =
  'T2JqAQQWYXZyby5zY2hlbWGQAnsidHlwZSI6InJlY29yZCIsIm5hbWUiOiJVc2VyIiwiZmllbGRzIjpbeyJuYW1lIjoibmFtZSIsInR5cGUiOiJzdHJpbmcifSx7Im5hbWUiOiJhZ2UiLCJ0eXBlIjoiaW50In0seyJuYW1lIjoiYWRtaW4iLCJ0eXBlIjoiYm9vbGVhbiJ9XX0UYXZyby5jb2RlYwhudWxsAJoySMYcwX2+PP0zY/z22TsEHAZBZGFIAQpMaW51c2wAmjJIxhzBfb48/TNj/PbZOw==';

test('avro-to-json decodes an OCF to a JSON array (default records)', async ({ page }) => {
  await page.goto('/tools/avro-to-json/');
  await page.fill('#in-input', OCF_B64);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "Ada"', { timeout: 15000 });
  await expect(out).toContainText('"age": 36');
  await expect(out).toContainText('"admin": true');
  await expect(out).toContainText('"name": "Linus"');
});

test('avro-to-json full format shows the embedded schema and count', async ({ page }) => {
  await page.goto('/tools/avro-to-json/');
  await page.fill('#in-input', OCF_B64);
  await page.selectOption('#in-format', 'full');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count": 2', { timeout: 15000 });
  await expect(out).toContainText('"schema"');
  await expect(out).toContainText('"name": "User"');
});

test('avro-to-json query-param deep-link prefills and decodes', async ({ page }) => {
  await page.goto('/tools/avro-to-json/?input=' + encodeURIComponent(OCF_B64) + '&format=ndjson');
  await expect(page.locator('#in-input')).toHaveValue(OCF_B64, { timeout: 15000 });
  // NDJSON: one compact record per line.
  await expect(page.locator('#tool-output')).toContainText('{"name":"Ada","age":36,"admin":true}', {
    timeout: 15000,
  });
});
