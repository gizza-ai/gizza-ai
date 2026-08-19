import { test, expect } from './fixtures';

async function runWasm(
  page,
  input: string,
  numbers = 'true',
  booleans = 'true',
  nulls = 'true',
  boolSynonyms = 'false',
  nullTokens = '',
  emptyStrings = 'keep',
  trim = 'false',
  leadingZeros = 'keep',
  thousands = 'false',
  skipKeys = '',
  onlyKeys = '',
  indent = '2',
  output = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-coerce-types/gizza_ai_json_coerce_types_web.js');
    await mod.default('/tools/json-coerce-types/gizza_ai_json_coerce_types_web_bg.wasm');
    return mod.run(
      args.input,
      args.numbers,
      args.booleans,
      args.nulls,
      args.boolSynonyms,
      args.nullTokens,
      args.emptyStrings,
      args.trim,
      args.leadingZeros,
      args.thousands,
      args.skipKeys,
      args.onlyKeys,
      args.indent,
      args.output,
    );
  }, { input, numbers, booleans, nulls, boolSynonyms, nullTokens, emptyStrings, trim, leadingZeros, thousands, skipKeys, onlyKeys, indent, output });
}

const DEFAULT_INPUT = '{"age":"42","active":"true","note":"null","zip":"02134"}';
const DEFAULT_OUTPUT = `{
  "age": 42,
  "active": true,
  "note": null,
  "zip": "02134"
}`;

test('json-coerce-types wasm coerces the default JSON exactly', async ({ page }) => {
  await page.goto('/tools/json-coerce-types/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, DEFAULT_INPUT)).resolves.toBe(DEFAULT_OUTPUT);
});

test('json-coerce-types wasm covers advertised enum values, checkboxes and key filters', async ({ page }) => {
  await page.goto('/tools/json-coerce-types/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(
    page,
    '{"a":"  1,234 ","b":"yes","c":"NA","d":"","e":"007","keep":{"n":"1"}}',
    'true',
    'true',
    'true',
    'true',
    'NA,N/A',
    'null',
    'true',
    'coerce',
    'true',
    'keep',
    '',
    '0',
    'json',
  )).resolves.toBe('{"a":1234,"b":true,"c":null,"d":null,"e":7,"keep":{"n":"1"}}');

  await expect(runWasm(
    page,
    '{"age":"42","counts":["1","2"],"meta":{"n":"3"}}',
    'true',
    'true',
    'true',
    'false',
    '',
    'keep',
    'false',
    'keep',
    'false',
    '',
    'counts',
    '0',
    'json',
  )).resolves.toBe('{"age":"42","counts":[1,2],"meta":{"n":"3"}}');
});

test('json-coerce-types page renders exact output and honors non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/json-coerce-types/');
  await page.fill('#in-input', '{"a":"1","b":"true","c":"null"}');
  await page.uncheck('#in-numbers');
  await page.uncheck('#in-booleans');
  await page.uncheck('#in-nulls');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText('{"a":"1","b":"true","c":"null"}', { timeout: 15_000 });
});

test('json-coerce-types deep-link prefills controls and returns a change report', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{"subscribed":"yes","amount":"1,234.50","blank":"","zip":"02134"}',
    numbers: 'true',
    booleans: 'true',
    nulls: 'true',
    bool_synonyms: 'true',
    null_tokens: 'NA,N/A,-',
    empty_strings: 'null',
    trim: 'true',
    leading_zeros: 'keep',
    thousands: 'true',
    skip_keys: 'zip',
    only_keys: '',
    indent: '2',
    output: 'report',
  });

  await page.goto(`/tools/json-coerce-types/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('{"subscribed":"yes","amount":"1,234.50","blank":"","zip":"02134"}', { timeout: 15_000 });
  await expect(page.locator('#in-bool_synonyms')).toBeChecked();
  await expect(page.locator('#in-thousands')).toBeChecked();
  await expect(page.locator('#in-empty_strings')).toHaveValue('null');
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#tool-output')).toHaveText(
    '3 string value(s) scanned, 3 coerced.\n\n$.subscribed: "yes" -> true\n$.amount: "1,234.50" -> 1234.5\n$.blank: "" -> null\n',
    { timeout: 15_000 },
  );
});
