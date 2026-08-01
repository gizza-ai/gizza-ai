import { test, expect } from './fixtures';

const yamlSpec = `openapi: "3.0.3"
components:
  schemas:
    Status:
      type: string
      enum: [active, banned]
    User:
      type: object
      required: [id]
      properties:
        id: { type: integer }
        status: { $ref: "#/components/schemas/Status" }
`;

test('openapi-to-typescript-types generates interfaces and union enums', async ({ page }) => {
  await page.goto('/tools/openapi-to-typescript-types/');
  await page.fill('#in-spec', yamlSpec);
  await expect(page.locator('#tool-output')).toHaveText(
    'export type Status = "active" | "banned";\n\nexport interface User {\n  id: number;\n  status?: Status;\n}',
    { timeout: 15000 },
  );
});

test('openapi-to-typescript-types honours a deep link with enum output and readonly props', async ({ page }) => {
  const spec = 'definitions:\n  Color:\n    type: string\n    enum: [red, green]\n';
  const qs =
    '?spec=' + encodeURIComponent(spec) +
    '&input_format=yaml' +
    '&enum_style=enum' +
    '&readonly=true' +
    '&indent=2';
  await page.goto('/tools/openapi-to-typescript-types/' + qs);
  await expect(page.locator('#in-input_format')).toHaveValue('yaml', { timeout: 15000 });
  await expect(page.locator('#in-enum_style')).toHaveValue('enum');
  await expect(page.locator('#in-readonly')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'export enum Color {\n  red = "red",\n  green = "green",\n}',
    { timeout: 15000 },
  );
});

test('openapi-to-typescript-types supports type aliases, sorted readonly required props', async ({ page }) => {
  await page.goto('/tools/openapi-to-typescript-types/');
  await page.fill('#in-spec', '{"components":{"schemas":{"Box":{"type":"object","properties":{"z":{"type":"string"},"a":{"type":"number"}}}}}}');
  await page.selectOption('#in-declaration', 'type');
  await page.selectOption('#in-optional_style', 'required');
  await page.check('#in-readonly');
  await page.check('#in-sort');
  await expect(page.locator('#tool-output')).toHaveText(
    'export type Box = {\n  readonly a: number;\n  readonly z: string;\n};',
    { timeout: 15000 },
  );
});
