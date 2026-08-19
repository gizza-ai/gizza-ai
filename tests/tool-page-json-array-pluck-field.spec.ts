import { test, expect } from './fixtures';

async function runWasm(
  page,
  json: string,
  field: string,
  root = '',
  format = 'lines',
  delimiter = ', ',
  quote = 'false',
  missing = 'skip',
  complexValues = 'json',
  unique = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-array-pluck-field/gizza_ai_json_array_pluck_field_web.js');
    await mod.default('/tools/json-array-pluck-field/gizza_ai_json_array_pluck_field_web_bg.wasm');
    return mod.run(
      args.json,
      args.field,
      args.root,
      args.format,
      args.delimiter,
      args.quote,
      args.missing,
      args.complexValues,
      args.unique,
    );
  }, { json, field, root, format, delimiter, quote, missing, complexValues, unique });
}

test('json-array-pluck-field wasm extracts nested values exactly', async ({ page }) => {
  await page.goto('/tools/json-array-pluck-field/');
  await page.waitForSelector('#in-json');

  const json = '[{"user":{"name":"Ada"},"email":"ada@example.test"},{"user":{"name":"Grace"},"email":"grace@example.test"}]';
  await expect(runWasm(page, json, 'user.name')).resolves.toBe('Ada\nGrace');
  await expect(runWasm(page, json, 'email', '', 'csv')).resolves.toBe('ada@example.test,grace@example.test');
});

test('json-array-pluck-field wasm covers enums, checkbox, boundary, and secondary inputs', async ({ page }) => {
  await page.goto('/tools/json-array-pluck-field/');
  await page.waitForSelector('#in-json');

  const wrapped = '{"items":[{"id":101,"tags":["a","b"]},{"id":102,"tags":["c"]},{"id":101,"tags":["d"]}]}';
  await expect(runWasm(page, wrapped, 'id', 'items', 'json', ', ', 'false', 'skip', 'json', 'true')).resolves.toBe('[101,102]');
  await expect(runWasm(page, wrapped, 'tags', 'items', 'lines', ', ', 'false', 'skip', 'label')).resolves.toBe('[array]\n[array]\n[array]');
  await expect(runWasm(page, wrapped, 'missing', 'items', 'custom', ' | ', 'true', 'empty')).resolves.toBe('"" | "" | ""');

  const ndjson = '{"sku":"A1"}\n{"sku":"B2"}';
  await expect(runWasm(page, ndjson, 'sku', '', 'tsv')).resolves.toBe('A1\tB2');

  const fieldAtCap = 'a'.repeat(200);
  await expect(runWasm(page, '[{"a":1}]', fieldAtCap)).resolves.toBe('');
  const fieldOverCap = 'a'.repeat(201);
  await expect(runWasm(page, '[{"a":1}]', fieldOverCap)).rejects.toThrow(/over the 200-byte limit/);
});

test('json-array-pluck-field page renders output from controls', async ({ page }) => {
  await page.goto('/tools/json-array-pluck-field/');
  await page.fill('#in-json', '[{"user":{"name":"Ada"}},{"user":{"name":"Grace"}}]');
  await page.fill('#in-field', 'user.name');
  await page.selectOption('#in-format', 'lines');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Ada', { timeout: 15_000 });
  await expect(out).toContainText('Grace');
});

test('json-array-pluck-field deep-link prefills values and renders', async ({ page }) => {
  const params = new URLSearchParams({
    json: '{"items":[{"id":101},{"id":102},{"id":101}]}',
    field: 'id',
    root: 'items',
    format: 'json',
    delimiter: ', ',
    quote: 'false',
    missing: 'skip',
    complex_values: 'json',
    unique: 'true',
  });

  await page.goto(`/tools/json-array-pluck-field/?${params.toString()}`);
  await expect(page.locator('#in-json')).toHaveValue('{"items":[{"id":101},{"id":102},{"id":101}]}', { timeout: 15_000 });
  await expect(page.locator('#in-field')).toHaveValue('id');
  await expect(page.locator('#in-root')).toHaveValue('items');
  await expect(page.locator('#in-unique')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('[101,102]', { timeout: 15_000 });
});
