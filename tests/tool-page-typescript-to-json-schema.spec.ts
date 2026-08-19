import { test, expect } from './fixtures';

const INPUT = 'interface User { id: number; name?: string; role: "admin" | "user" }';

async function outputText(page: any): Promise<string> {
  const output = page.locator('#tool-output');
  await expect(output).toContainText('"$schema"', { timeout: 15_000 });
  return (await output.textContent()) ?? '';
}

test('typescript-to-json-schema page converts an interface with optional props and literal union', async ({ page }) => {
  await page.goto('/tools/typescript-to-json-schema/');
  await page.waitForSelector('#in-typescript');
  await page.fill('#in-typescript', INPUT);
  await page.fill('#in-root_type', 'User');
  await page.selectOption('#in-draft', '2020-12');
  const text = await outputText(page);
  const schema = JSON.parse(text);
  expect(schema.$schema).toBe('https://json-schema.org/draft/2020-12/schema');
  expect(schema.title).toBe('User');
  expect(schema.properties.id.type).toBe('number');
  expect(schema.properties.name.type).toBe('string');
  expect(schema.properties.role).toEqual({ type: 'string', enum: ['admin', 'user'] });
  expect(schema.required).toEqual(['id', 'role']);
  expect(schema.additionalProperties).toBe(false);
});

test('typescript-to-json-schema deep link supports Draft-07 and allow-extra-properties toggle', async ({ page }) => {
  const params = new URLSearchParams({
    typescript: 'interface Bag { tags: string[]; count?: number }',
    root_type: 'Bag',
    draft: 'draft-07',
    required: 'true',
    additional_properties: 'true',
    jsdoc: 'true',
  });
  await page.goto(`/tools/typescript-to-json-schema/?${params.toString()}`);
  await page.waitForSelector('#in-typescript');
  await expect(page.locator('#in-root_type')).toHaveValue('Bag', { timeout: 15_000 });
  await expect(page.locator('#in-draft')).toHaveValue('draft-07');
  await expect(page.locator('#in-additional_properties')).toBeChecked();
  const text = await outputText(page);
  const schema = JSON.parse(text);
  expect(schema.$schema).toBe('http://json-schema.org/draft-07/schema#');
  expect(schema.properties.tags).toEqual({ type: 'array', items: { type: 'string' } });
  expect(schema.required).toEqual(['tags']);
  expect(schema).not.toHaveProperty('additionalProperties');
});
