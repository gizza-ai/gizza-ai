import { test, expect } from './fixtures';

const jsonSpec = JSON.stringify({
  swagger: '2.0',
  info: { title: 'Pet Store', version: '1.0.0' },
  host: 'api.example.com',
  basePath: '/v1',
  schemes: ['https'],
  paths: {
    '/pets': {
      get: {
        parameters: [{ name: 'limit', in: 'query', type: 'integer', format: 'int32' }],
        responses: {
          '200': { description: 'A list of pets.', schema: { $ref: '#/definitions/Pet' } },
        },
      },
    },
  },
  definitions: {
    Pet: { type: 'object', required: ['id'], properties: { id: { type: 'integer' } } },
  },
}, null, 2);

const yamlSpec = `swagger: "2.0"
info:
  title: Pet Store
  version: 1.0.0
host: api.example.com
basePath: /v1
schemes:
  - https
paths:
  /pets:
    get:
      responses:
        "200":
          description: A list of pets.
`;

test('swagger2-to-openapi3 page converts JSON Swagger to OpenAPI JSON', async ({ page }) => {
  await page.goto('/tools/swagger2-to-openapi3/');
  await page.fill('#in-spec', jsonSpec);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"openapi": "3.0.3"', { timeout: 15_000 });
  await expect(out).toContainText('"url": "https://api.example.com/v1"');
  await expect(out).toContainText('"components"');
  await expect(out).toContainText('"$ref": "#/components/schemas/Pet"');
  await expect(out).toContainText('"schema": {');
});

test('swagger2-to-openapi3 deep link applies target version and minified indent', async ({ page }) => {
  const qs =
    '?spec=' + encodeURIComponent('{"swagger":"2.0","info":{"title":"API","version":"1"},"paths":{}}') +
    '&input_format=json&output_format=json&target_version=3.0.0&indent=0';
  await page.goto('/tools/swagger2-to-openapi3/' + qs);
  await expect(page.locator('#in-target_version')).toHaveValue('3.0.0', { timeout: 15_000 });
  await expect(page.locator('#in-indent')).toHaveValue('0');
  const text = await page.locator('#tool-output').textContent();
  expect(text).toContain('{"openapi":"3.0.0"');
  expect(text).not.toContain('\n  "info"');
});

test('swagger2-to-openapi3 page emits YAML when requested', async ({ page }) => {
  await page.goto('/tools/swagger2-to-openapi3/');
  await page.fill('#in-spec', yamlSpec);
  await page.selectOption('#in-input_format', 'yaml');
  await page.selectOption('#in-output_format', 'yaml');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("openapi: '3.0.3'", { timeout: 15_000 });
  await expect(out).toContainText('url: https://api.example.com/v1');
});

test('swagger2-to-openapi3 page patch checkbox can be disabled', async ({ page }) => {
  await page.goto('/tools/swagger2-to-openapi3/');
  await page.fill('#in-spec', '{"swagger":"2.0","info":{},"paths":{}}');
  await page.uncheck('#in-patch');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"openapi": "3.0.3"', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('"info": {}');
  expect(text).not.toContain('"title": "API"');
});

test('swagger2-to-openapi3 page reports non-Swagger input clearly', async ({ page }) => {
  await page.goto('/tools/swagger2-to-openapi3/');
  await page.fill('#in-spec', '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('OpenAPI 3.x document already', { timeout: 15_000 });
  await expect(out).toHaveClass(/error/);
});
