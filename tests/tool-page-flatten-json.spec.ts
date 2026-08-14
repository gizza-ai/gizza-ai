import { test, expect } from './fixtures';

const DOC = '{"user":{"name":"Ada","tags":["admin","beta"]},"active":true}';
const EXACT = '{"user.name":"Ada","user.tags[0]":"admin","user.tags[1]":"beta","active":true}';
const FORM_EXACT = '{"user.name":"Ada","user.tags":["admin","beta"],"active":true}';

async function runWasm(
  page: any,
  json = DOC,
  direction = 'flatten',
  separator = '.',
  array_notation = 'bracket',
  max_depth = 0,
  flatten_arrays = true,
  preserve_empty = true,
  key_case = 'preserve',
  output = 'json',
  pretty = false,
  indent = 2,
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/flatten-json/gizza_ai_flatten_json_web.js');
    await mod.default('/tools/flatten-json/gizza_ai_flatten_json_web_bg.wasm');
    return mod.run(
      args.json,
      args.direction,
      args.separator,
      args.array_notation,
      args.max_depth,
      args.flatten_arrays,
      args.preserve_empty,
      args.key_case,
      args.output,
      args.pretty,
      args.indent,
    );
  }, { json, direction, separator, array_notation, max_depth, flatten_arrays, preserve_empty, key_case, output, pretty, indent });
}

test('flatten-json wasm flattens and unflattens exact JSON', async ({ page }) => {
  await page.goto('/tools/flatten-json/');
  await page.waitForSelector('#in-json');

  expect(await runWasm(page)).toBe(EXACT);
  expect(await runWasm(page, EXACT, 'unflatten')).toBe(DOC);
  expect(await runWasm(page, '{"db":{"host":"local"}}', 'flatten', '_', 'bracket', 0, true, true, 'upper', 'pairs'))
    .toBe('DB_HOST=local');
});

test('flatten-json page computes exact compact output from the form', async ({ page }) => {
  await page.goto('/tools/flatten-json/');
  await page.fill('#in-json', DOC);
  await page.selectOption('#in-direction', 'flatten');
  await page.selectOption('#in-array_notation', 'bracket');
  await page.selectOption('#in-output', 'json');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText(FORM_EXACT, { timeout: 15_000 });
});

test('flatten-json deep link covers unflatten, separator notation, and pretty checkbox off', async ({ page }) => {
  const params = new URLSearchParams({
    json: '{"rows.0.id":1,"rows.1.id":2}',
    direction: 'unflatten',
    separator: '.',
    array_notation: 'separator',
    max_depth: '0',
    flatten_arrays: 'true',
    preserve_empty: 'true',
    key_case: 'preserve',
    output: 'json',
    pretty: 'false',
    indent: '2',
  });
  await page.goto(`/tools/flatten-json/?${params.toString()}`);

  await expect(page.locator('#in-direction')).toHaveValue('unflatten', { timeout: 15_000 });
  await expect(page.locator('#in-array_notation')).toHaveValue('separator');
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('{"rows":[{"id":1},{"id":2}]}', { timeout: 15_000 });
});

test('flatten-json covers advertised modes, non-default booleans, cap boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/flatten-json/');
  await page.waitForSelector('#in-json');

  expect(await runWasm(page, '{"a":{"list":[1,{"x":2}]}}', 'flatten', '.', 'bracket', 0, false, true, 'preserve', 'json', false))
    .toBe('{"a.list":[1,{"x":2}]}');
  expect(await runWasm(page, '{"a":{},"b":[],"c":1}', 'flatten', '.', 'bracket', 0, true, false, 'preserve', 'paths'))
    .toBe('c');
  expect(await runWasm(page, '{"a":"x,y","b":"say \\"hi\\""}', 'flatten', '.', 'bracket', 0, true, true, 'preserve', 'csv'))
    .toBe('key,value\na,"x,y"\nb,"say ""hi"""');
  expect(await runWasm(page, '{"a":{"b":{"c":1}}}', 'flatten', '.', 'bracket', 2, true, true, 'preserve', 'json', false))
    .toBe('{"a.b":{"c":1}}');

  const deep = `{"${'a.'.repeat(100)}leaf":1}`;
  await expect(runWasm(page, deep, 'unflatten')).rejects.toThrow(/more than 100 path segments/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool flatten-json');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
