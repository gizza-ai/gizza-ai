import { test, expect } from './fixtures';

// /tools/json-to-json-schema/ infers JSON Schema from pasted JSON examples.

const SAMPLE = '[{"id":1,"email":"ada@example.com"},{"id":2,"email":"grace@example.com","admin":true}]';

async function outputJson(page) {
  const text = (await page.locator('#tool-output').textContent())?.trim() ?? '';
  return JSON.parse(text);
}

test('json-to-json-schema infers array item schema with exact fields', async ({ page }) => {
  await page.goto('/tools/json-to-json-schema/');
  await page.fill('#in-json', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"$schema": "https://json-schema.org/draft/2020-12/schema"', {
    timeout: 15000,
  });
  const schema = await outputJson(page);
  expect(schema.type).toBe('array');
  expect(schema.items.type).toBe('object');
  expect(schema.items.properties.id.type).toBe('integer');
  expect(schema.items.properties.email).toEqual({ type: 'string', format: 'email' });
  expect(schema.items.properties.admin.type).toBe('boolean');
  expect(schema.items.required).toEqual(['email', 'id']);
  expect(schema.items.additionalProperties).toBe(false);
});

test('json-to-json-schema supports Draft-07, title, and permissive objects', async ({ page }) => {
  await page.goto('/tools/json-to-json-schema/');
  await page.fill('#in-json', '{"id":"12345678-1234-1234-1234-1234567890ab","name":"Ada"}');
  await page.selectOption('#in-draft', 'draft-07');
  await page.check('#in-additional_properties');
  await page.fill('#in-title', 'User');
  await expect(page.locator('#tool-output')).toContainText('"$schema": "http://json-schema.org/draft-07/schema#"', {
    timeout: 15000,
  });
  const schema = await outputJson(page);
  expect(schema.title).toBe('User');
  expect(schema.additionalProperties).toBeUndefined();
  expect(schema.properties.id.type).toBe('string');
  expect(schema.properties.id.format).toBeUndefined();
});

test('json-to-json-schema can disable required fields and format detection', async ({ page }) => {
  await page.goto('/tools/json-to-json-schema/');
  await page.fill('#in-json', '{"email":"ada@example.com","created":"2020-01-02"}');
  await page.uncheck('#in-required');
  await page.uncheck('#in-detect_formats');
  await expect(page.locator('#tool-output')).toContainText('"email"', { timeout: 15000 });
  const schema = await outputJson(page);
  expect(schema.required).toBeUndefined();
  expect(schema.properties.email).toEqual({ type: 'string' });
  expect(schema.properties.created).toEqual({ type: 'string' });
});

test('json-to-json-schema deep-link pre-fills and runs', async ({ page }) => {
  const q = encodeURIComponent('{"name":"Ada","age":30}');
  await page.goto(`/tools/json-to-json-schema/?json=${q}&title=Person&draft=2020-12`);
  await expect(page.locator('#tool-output')).toContainText('"title": "Person"', { timeout: 15000 });
  const schema = await outputJson(page);
  expect(schema.properties.age.type).toBe('integer');
});

test('json-to-json-schema reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-to-json-schema/');
  await page.fill('#in-json', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});
