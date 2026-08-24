import { test, expect } from './fixtures';

const SAMPLE = '{"name":"  Ada  ","city":" Berlin ","tags":[" x ","y  "],"visits":7}';
const PRETTY = `{
  "name": "Ada",
  "city": "Berlin",
  "tags": [
    "x",
    "y"
  ],
  "visits": 7
}`;

async function runWasm(
  page: any,
  input = SAMPLE,
  trim = 'both',
  internal = 'keep',
  whitespace = 'unicode',
  keys = 'false',
  empty = 'keep',
  onlyKeys = '',
  skipKeys = '',
  indent = '2',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-trim-strings/gizza_ai_json_trim_strings_web.js');
    await mod.default('/tools/json-trim-strings/gizza_ai_json_trim_strings_web_bg.wasm');
    return mod.run(
      args.input,
      args.trim,
      args.internal,
      args.whitespace,
      args.keys,
      args.empty,
      args.onlyKeys,
      args.skipKeys,
      args.indent,
    );
  }, { input, trim, internal, whitespace, keys, empty, onlyKeys, skipKeys, indent });
}

test('json-trim-strings wasm trims every string value exactly', async ({ page }) => {
  await page.goto('/tools/json-trim-strings/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page)).resolves.toBe(PRETTY);
});

test('json-trim-strings wasm covers enum values, checkboxes, filters, and errors', async ({ page }) => {
  await page.goto('/tools/json-trim-strings/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, '{"name":"  Ada   Lovelace  ","sku":" AB 12  CD "}', 'both', 'collapse', 'unicode', 'false', 'keep', '', '', '0'))
    .resolves.toBe('{"name":"Ada Lovelace","sku":"AB 12 CD"}');

  await expect(runWasm(page, '{" sku ":" AB 12  CD ","note":" keep  me "}', 'both', 'remove', 'unicode', 'true', 'keep', '', 'note', '0'))
    .resolves.toBe('{"sku":"AB12CD","note":" keep  me "}');

  await expect(runWasm(page, '{"a":"   ","b":" x ","c":""}', 'both', 'keep', 'unicode', 'false', 'drop', '', '', '0'))
    .resolves.toBe('{"b":"x","c":""}');

  await expect(runWasm(page, '{" a ":1,"a":2}', 'both', 'keep', 'unicode', 'true', 'keep', '', '', '0'))
    .rejects.toThrow(/key collision/);
});

test('json-trim-strings page renders exact output and honors non-default controls', async ({ page }) => {
  await page.goto('/tools/json-trim-strings/');
  await page.fill('#in-input', '{"name":"  Ada   Lovelace  ","code":" AB 12  CD ","note":" keep  me "}');
  await page.selectOption('#in-internal', 'remove');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText('{"name":"AdaLovelace","code":"AB12CD","note":"keepme"}', { timeout: 15_000 });
});

test('json-trim-strings deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{" first name ":" Ada ","middle":"   ","tags":[" x "," y "]}',
    trim: 'both',
    internal: 'keep',
    whitespace: 'unicode',
    keys: 'true',
    empty: 'null',
    only_keys: '',
    skip_keys: '',
    indent: '0',
  });

  await page.goto(`/tools/json-trim-strings/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('{" first name ":" Ada ","middle":"   ","tags":[" x "," y "]}', { timeout: 15_000 });
  await expect(page.locator('#in-keys')).toBeChecked();
  await expect(page.locator('#in-empty')).toHaveValue('null');
  await expect(page.locator('#tool-output')).toHaveText('{"first name":"Ada","middle":null,"tags":["x","y"]}', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool json-trim-strings');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
