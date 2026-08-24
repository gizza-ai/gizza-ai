import { test, expect } from './fixtures';

const requestJson = '{"name":"Ada Lovelace","email":"ada@example.com","active":true}';
const responseJson = '{"id":7,"name":"Ada Lovelace","email":"ada@example.com","created_at":"2026-08-21T07:00:00Z"}';

test('openapi-stub-from-json generates a YAML operation with component schemas', async ({ page }) => {
  await page.goto('/tools/openapi-stub-from-json/');
  await page.fill('#in-request_json', requestJson);
  await page.fill('#in-response_json', responseJson);
  await page.fill('#in-path', '/users');
  await page.fill('#in-operation_id', 'createUser');
  await page.fill('#in-tag', 'Users');
  await page.fill('#in-title', 'Users API');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("openapi: '3.1.0'", { timeout: 15000 });
  await expect(out).toContainText('operationId: createUser');
  await expect(out).toContainText("$ref: '#/components/schemas/CreateUserRequest'");
  await expect(out).toContainText('format: email');
  await expect(out).toContainText('format: date-time');
});

test('openapi-stub-from-json supports deep-linked JSON output with security and errors', async ({ page }) => {
  const params = new URLSearchParams({
    request_json: '{"email":"ada@example.com"}',
    response_json: '{"ok":true}',
    path: '/sessions',
    security: 'apiKey',
    format: 'json',
    include_error_responses: 'true',
  });
  await page.goto(`/tools/openapi-stub-from-json/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"openapi": "3.1.0"', { timeout: 15000 });
  await expect(out).toContainText('"ApiKeyAuth"');
  await expect(out).toContainText('"400"');
  await expect(out).toContainText('"500"');
});

test('openapi-stub-from-json can inline schemas and omit examples', async ({ page }) => {
  await page.goto('/tools/openapi-stub-from-json/');
  await page.fill('#in-response_json', '[{"id":1,"name":"Ada"},{"id":2,"name":"Grace","admin":false}]');
  await page.selectOption('#in-method', 'get');
  await page.fill('#in-path', '/users');
  await page.uncheck('#in-components');
  await page.uncheck('#in-include_examples');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('/users:', { timeout: 15000 });
  await expect(out).toContainText('type: array');
  await expect(out).toContainText('admin:');
  await expect(out).not.toContainText('components:');
  await expect(out).not.toContainText('example:');
});
