import { test, expect } from './fixtures';

const tool = '/tools/keyvalue-text-parser/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  input: string,
  separator = 'auto',
  customSeparator = '',
  structure = 'object',
  duplicates = 'group',
  trim = 'true',
  unquote = 'true',
  commentPrefixes = '#,;,//',
  inferTypes = 'false',
  keyCase = 'as-is',
  unmatched = 'skip',
  indent = '2',
): Promise<string> {
  return await page.evaluate(
    async (args) => {
      const mod = await import('/tools/keyvalue-text-parser/gizza_ai_keyvalue_text_parser_web.js');
      await mod.default('/tools/keyvalue-text-parser/gizza_ai_keyvalue_text_parser_web_bg.wasm');
      return mod.run(
        args.input,
        args.separator,
        args.customSeparator,
        args.structure,
        args.duplicates,
        args.trim,
        args.unquote,
        args.commentPrefixes,
        args.inferTypes,
        args.keyCase,
        args.unmatched,
        args.indent,
      );
    },
    { input, separator, customSeparator, structure, duplicates, trim, unquote, commentPrefixes, inferTypes, keyCase, unmatched, indent },
  );
}

test('keyvalue-text-parser page groups repeated keys into exact JSON', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-input'), 'Name: Ada\nrole = engineer\ntag: math\ntag: computing');
  await page.selectOption('#in-separator', 'auto');
  await page.selectOption('#in-structure', 'object');
  await page.selectOption('#in-duplicates', 'group');
  await page.check('#in-trim');
  await page.check('#in-unquote');
  await page.uncheck('#in-infer_types');
  await page.selectOption('#in-key_case', 'as-is');
  await page.selectOption('#in-unmatched', 'skip');
  await page.fill('#in-indent', '2');

  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "Name": "Ada",\n  "role": "engineer",\n  "tag": [\n    "math",\n    "computing"\n  ]\n}',
    { timeout: 15000 },
  );
});

test('keyvalue-text-parser deep link uses records, type inference and snake case', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'Name: Ada\nActive: true\n\nName: Grace\nActive: false',
    separator: 'colon',
    custom_separator: '',
    structure: 'records',
    duplicates: 'last',
    trim: 'true',
    unquote: 'true',
    comment_prefixes: '#,;,//',
    infer_types: 'true',
    key_case: 'snake',
    unmatched: 'skip',
    indent: '0',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-structure')).toHaveValue('records', { timeout: 15000 });
  await expect(page.locator('#in-duplicates')).toHaveValue('last');
  await expect(page.locator('#in-infer_types')).toBeChecked();
  await expect(page.locator('#in-key_case')).toHaveValue('snake');
  await expect(page.locator('#in-indent')).toHaveValue('0');
  await expect(page.locator('#tool-output')).toHaveText('[{"name":"Ada","active":true},{"name":"Grace","active":false}]');
});

test('keyvalue-text-parser wasm covers separators duplicate policies and strict errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, 'host\tlocal:host', 'tab', '', 'object', 'group', 'true', 'true', '', 'false', 'as-is', 'skip', '0')).toBe('{"host":"local:host"}');
  expect(await runWasm(page, 'host|example.com\nport|443', 'pipe', '', 'object', 'last', 'true', 'true', '', 'true', 'as-is', 'skip', '0')).toBe('{"host":"example.com","port":443}');
  expect(await runWasm(page, 'sku -> A-001\nqty -> 7', 'custom', '->', 'pairs', 'group', 'true', 'true', '', 'true', 'lower', 'skip', '0')).toBe('[{"key":"sku","value":"A-001","line":1},{"key":"qty","value":7,"line":2}]');
  expect(await runWasm(page, 'k: a\nk: b', 'colon', '', 'object', 'first', 'true', 'true', '', 'false', 'as-is', 'skip', '0')).toBe('{"k":"a"}');
  await expect(runWasm(page, 'k: a\nk: b', 'colon', '', 'object', 'error')).rejects.toThrow(/duplicate key 'k' on line 2/);
  await expect(runWasm(page, 'plain text', 'custom', '', 'object', 'group')).rejects.toThrow(/needs a custom_separator/);
  await expect(runWasm(page, 'a: 1\nplain text', 'colon', '', 'object', 'group', 'true', 'true', '', 'false', 'as-is', 'error')).rejects.toThrow(/line 2 has no separator/);
});

test('keyvalue-text-parser enforces the advertised 10000-line cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/keyvalue-text-parser/gizza_ai_keyvalue_text_parser_web.js');
    await mod.default('/tools/keyvalue-text-parser/gizza_ai_keyvalue_text_parser_web_bg.wasm');
    const atCap = Array.from({ length: 10000 }, (_, i) => `k${i}: ${i}`).join('\n');
    const overCap = atCap + '\nover: cap';
    const call = (input: string) => {
      try {
        return { ok: true, value: mod.run(input, 'auto', '', 'object', 'last', 'true', 'true', '', 'false', 'as-is', 'skip', '0').slice(0, 8) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('{"k0":"0');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/over the 10000-line limit/);
});

test('keyvalue-text-parser page ships workflow example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Metadata with repeated keys',
    'Blank-line records',
    'Pipe-delimited pairs',
    'Strict custom arrow',
  ]);
});
