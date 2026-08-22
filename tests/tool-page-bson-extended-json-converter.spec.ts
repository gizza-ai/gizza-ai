import { test, expect } from './fixtures';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('bson-extended-json-converter unwraps Extended JSON to plain JSON', async ({ page }) => {
  await page.goto('/tools/bson-extended-json-converter/');
  await page.fill('#in-input', '{"_id":{"$oid":"507f1f77bcf86cd799439011"},"created":{"$date":{"$numberLong":"1721485800000"}},"views":{"$numberLong":"42"},"name":"Ada"}');
  await page.selectOption('#in-direction', 'to-plain');

  await expect(page.locator('#tool-output')).toContainText('"_id": "507f1f77bcf86cd799439011"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toBe(`{
  "_id": "507f1f77bcf86cd799439011",
  "created": "2024-07-20T14:30:00Z",
  "views": 42,
  "name": "Ada"
}`);
});

test('bson-extended-json-converter wraps plain JSON as canonical Extended JSON', async ({ page }) => {
  await page.goto('/tools/bson-extended-json-converter/');
  await page.fill('#in-input', '{"_id":"507f1f77bcf86cd799439011","created":"2024-07-20T14:30:00Z","views":42,"score":1.5}');
  await page.selectOption('#in-direction', 'to-extended');
  await page.selectOption('#in-mode', 'canonical');
  await page.check('#in-detect_types');

  await expect(page.locator('#tool-output')).toContainText('"$oid": "507f1f77bcf86cd799439011"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"created": {\n    "$date": {\n      "$numberLong": "1721485800000"\n    }\n  }');
  expect(text).toContain('"views": {\n    "$numberInt": "42"\n  }');
  expect(text).toContain('"score": {\n    "$numberDouble": "1.5"\n  }');
});

test('bson-extended-json-converter deep-link supports compact epoch-millis output', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{"at":{"$date":{"$numberLong":"1721485800000"}},"n":{"$numberLong":"9007199254740993"}}',
    direction: 'to-plain',
    date_format: 'epoch-millis',
    big_numbers_as_strings: 'true',
    pretty: 'false',
  });

  await page.goto(`/tools/bson-extended-json-converter/?${params.toString()}`);
  await expect(page.locator('#in-direction')).toHaveValue('to-plain');
  await expect(page.locator('#in-date_format')).toHaveValue('epoch-millis');
  await expect(page.locator('#in-big_numbers_as_strings')).toBeChecked();
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('{"at":1721485800000,"n":"9007199254740993"}', { timeout: 15000 });
});

test('bson-extended-json-converter reports invalid wrapper values', async ({ page }) => {
  await page.goto('/tools/bson-extended-json-converter/');
  await page.fill('#in-input', '{"_id":{"$oid":"not-an-object-id"}}');
  await page.selectOption('#in-direction', 'to-plain');
  await expect(page.locator('#tool-output')).toContainText('ObjectId must be 24 hex characters', { timeout: 15000 });
});
