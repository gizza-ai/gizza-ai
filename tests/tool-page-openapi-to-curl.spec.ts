import { test, expect } from './fixtures';

const SPEC = `openapi: 3.0.3
servers:
  - url: https://petstore.example.com/v1
paths:
  /pets/{petId}:
    get:
      summary: Get a pet
      tags: [pets]
      parameters:
        - name: petId
          in: path
          required: true
          schema: { type: integer, example: 42 }
        - name: verbose
          in: query
          schema: { type: boolean, default: true }
      responses:
        '200': { description: OK }
  /pets:
    post:
      summary: Create a pet
      tags: [pets]
      requestBody:
        content:
          application/json:
            schema:
              type: object
              required: [name]
              properties:
                name: { type: string, example: Fido }
                age: { type: integer, default: 3 }
      responses:
        '201': { description: Created }
`;

test('openapi-to-curl page generates real curl commands', async ({ page }) => {
  await page.goto('/tools/openapi-to-curl/');
  await page.fill('#in-spec', SPEC);
  await page.selectOption('#in-input_format', 'yaml');
  await page.selectOption('#in-auth', 'bearer');
  await page.selectOption('#in-output_format', 'commands');
  await page.uncheck('#in-multiline');
  await page.fill('#in-methods', 'get');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('curl -X GET', { timeout: 20_000 });
  await expect(out).toContainText('https://petstore.example.com/v1/pets/42');
  await expect(out).toContainText("Authorization");
  await expect(out).not.toContainText('POST');
});

test('openapi-to-curl page honors deep-linked params and optional fields', async ({ page }) => {
  await page.goto('/tools/openapi-to-curl/?input_format=yaml&output_format=markdown&include_optional=true&max_depth=5');
  await expect(page.locator('#in-input_format')).toHaveValue('yaml');
  await expect(page.locator('#in-output_format')).toHaveValue('markdown');
  await expect(page.locator('#in-include_optional')).toBeChecked();
  await expect(page.locator('#in-max_depth')).toHaveValue('5');

  await page.fill('#in-spec', SPEC);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('## GET /pets/{petId}', { timeout: 20_000 });
  await expect(out).toContainText('```bash');
  await expect(out).toContainText('curl -X POST');
  await expect(out).toContainText('"age":3');
});
