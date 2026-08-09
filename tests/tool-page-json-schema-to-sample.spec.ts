import { test, expect } from './fixtures';

const USER_SCHEMA = JSON.stringify({
  type: 'object',
  properties: {
    id: { type: 'integer', minimum: 1 },
    email: { type: 'string', format: 'email' },
    nickname: { type: 'string' },
  },
  required: ['id', 'email'],
}, null, 2);

test('json-schema-to-sample page generates required fields', async ({ page }) => {
  await page.goto('/tools/json-schema-to-sample/');
  await page.fill('#in-schema', USER_SCHEMA);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"id": 1', { timeout: 15000 });
  await expect(out).toContainText('"email": "user@example.com"');
  await expect(out).not.toContainText('nickname');
});

test('json-schema-to-sample page deep-link can include optional fields compactly', async ({ page }) => {
  const params = new URLSearchParams({
    schema: USER_SCHEMA,
    include_optional: 'true',
    array_items: '1',
    pretty: 'false',
  });
  await page.goto(`/tools/json-schema-to-sample/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"id":1,"email":"user@example.com","nickname":"string"}', { timeout: 15000 });
});

test('json-schema-to-sample page reports invalid JSON Schema input', async ({ page }) => {
  await page.goto('/tools/json-schema-to-sample/');
  await page.fill('#in-schema', '{oops');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('schema is not valid JSON', { timeout: 15000 });
});
