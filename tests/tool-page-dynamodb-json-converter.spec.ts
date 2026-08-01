import { test, expect } from './fixtures';

test('dynamodb-json-converter marshals plain JSON to typed AttributeValue JSON', async ({ page }) => {
  await page.goto('/tools/dynamodb-json-converter/');
  await page.fill('#in-input', '{"id":"user#1","age":30,"active":true,"tags":["admin","user"]}');
  await page.selectOption('#in-direction', 'to-dynamodb');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"id": {', { timeout: 15000 });
  await expect(out).toContainText('"S": "user#1"');
  await expect(out).toContainText('"age": {');
  await expect(out).toContainText('"N": "30"');
  await expect(out).toContainText('"BOOL": true');
  await expect(out).toContainText('"L": [');
});

test('dynamodb-json-converter unmarshals sets and supports compact output', async ({ page }) => {
  await page.goto('/tools/dynamodb-json-converter/');
  await page.fill('#in-input', '{"roles":{"SS":["admin","user"]},"scores":{"NS":["1","2.5"]},"blob":{"B":"ZGF0YQ=="}}');
  await page.selectOption('#in-direction', 'from-dynamodb');
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"roles":["admin","user"]', { timeout: 15000 });
  await expect(out).toContainText('"scores":[1,2.5]');
  await expect(out).toContainText('"blob":"ZGF0YQ=="');
});

test('dynamodb-json-converter supports deep-linked auto-detect', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{"profile":{"M":{"name":{"S":"Ada"},"level":{"N":"3"}}},"deleted":{"NULL":true}}',
    direction: 'auto',
    pretty: 'false',
  });
  await page.goto(`/tools/dynamodb-json-converter/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"deleted":null', { timeout: 15000 });
  await expect(out).toContainText('"profile"');
  await expect(out).toContainText('"name":"Ada"');
  await expect(out).toContainText('"level":3');
});
