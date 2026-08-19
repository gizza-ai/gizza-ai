import { test, expect } from './fixtures';

const tool = '/tools/csv-regex-replace/';
const sample = 'name,city\n"Lovelace, Ada",Paris\n"Hopper, Grace",Boston';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  data: string,
  pattern: string,
  replacement = '',
  columns = '',
  mode = 'regex',
  matchScope = 'substring',
  ignoreCase = 'false',
  multiline = 'false',
  dotall = 'false',
  replaceAll = 'true',
  hasHeader = 'true',
  includeHeader = 'false',
  delimiter = 'auto',
  quoteStyle = 'minimal',
  output = 'csv',
) {
  return await page.evaluate(
    async ({
      data,
      pattern,
      replacement,
      columns,
      mode,
      matchScope,
      ignoreCase,
      multiline,
      dotall,
      replaceAll,
      hasHeader,
      includeHeader,
      delimiter,
      quoteStyle,
      output,
    }) => {
      const mod = await import('/tools/csv-regex-replace/gizza_ai_csv_regex_replace_web.js');
      await mod.default('/tools/csv-regex-replace/gizza_ai_csv_regex_replace_web_bg.wasm');
      return mod.run(
        data,
        pattern,
        replacement,
        columns,
        mode,
        matchScope,
        ignoreCase,
        multiline,
        dotall,
        replaceAll,
        hasHeader,
        includeHeader,
        delimiter,
        quoteStyle,
        output,
      );
    },
    {
      data,
      pattern,
      replacement,
      columns,
      mode,
      matchScope,
      ignoreCase,
      multiline,
      dotall,
      replaceAll,
      hasHeader,
      includeHeader,
      delimiter,
      quoteStyle,
      output,
    },
  );
}

test('csv-regex-replace page rewrites selected column with exact capture-group output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', sample);
  await page.fill('#in-pattern', '(\\w+), (\\w+)');
  await page.fill('#in-replacement', '$2 $1');
  await page.fill('#in-columns', 'name');
  await page.selectOption('#in-mode', 'regex');
  await page.selectOption('#in-match_scope', 'substring');
  await page.check('#in-replace_all');
  await page.check('#in-has_header');
  await page.fill('#in-delimiter', 'auto');
  await page.selectOption('#in-quote_style', 'minimal');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('Ada Lovelace,Paris', { timeout: 15000 });
  expect((await outputText(page)).trim()).toBe('name,city\nAda Lovelace,Paris\nGrace Hopper,Boston');
});

test('csv-regex-replace deep link prefills report output and non-default checkbox states', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent('code,note\ncode,keep\nCODE,keep') +
      '&pattern=code&replacement=ID&columns=code&mode=regex&match_scope=substring' +
      '&ignore_case=true&multiline=false&dotall=false&replace_all=false&has_header=true&include_header=true' +
      '&delimiter=auto&quote_style=always&output=report',
  );

  await expect(page.locator('#in-data')).toHaveValue('code,note\ncode,keep\nCODE,keep', { timeout: 15000 });
  await expect(page.locator('#in-ignore_case')).toBeChecked();
  await expect(page.locator('#in-replace_all')).not.toBeChecked();
  await expect(page.locator('#in-include_header')).toBeChecked();
  await expect(page.locator('#in-quote_style')).toHaveValue('always');
  await expect(page.locator('#in-output')).toHaveValue('report');

  await expect(page.locator('#tool-output')).toContainText('TOTAL');
  expect((await outputText(page)).trim()).toBe('"column","cells_changed","replacements"\n"code","3","3"\n"TOTAL","3","3"');
});

test('csv-regex-replace wasm covers advertised modes, scopes, delimiters, quoting and outputs', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, 'a\n1.5\n', '.', '$1,', '', 'literal')).toBe('a\n"1$1,5"\n');
  expect(await runWasm(page, 'a\nNA\nNAME\n', 'NA', '', '', 'regex', 'whole_cell')).toBe('a\n""\nNAME\n');
  expect(await runWasm(page, 'a\nx-x-x\n', 'x', 'y', '', 'regex', 'substring', 'false', 'false', 'false', 'false')).toBe('a\ny-x-x\n');
  expect(await runWasm(page, 'a\nHello\n', 'hello', 'hi', '', 'regex', 'substring', 'true')).toBe('a\nhi\n');
  expect(await runWasm(page, 'a\n"one\ntwo"\n', 'one.two', 'merged', '', 'regex', 'substring', 'false', 'false', 'true')).toBe('a\nmerged\n');
  expect(await runWasm(page, 'a\n"one\ntwo"\n', '^t', 'T', '', 'regex', 'substring', 'false', 'true')).toBe('a\n"one\nTwo"\n');
  expect(await runWasm(page, 'a\tb\nxx\txx\n', 'x', 'y', 'b', 'regex', 'substring', 'false', 'false', 'false', 'true', 'true', 'false', 'auto')).toBe('a\tb\nxx\tyy\n');
  expect(await runWasm(page, 'a,b\nx,y\n', 'x', 'z', '', 'regex', 'substring', 'false', 'false', 'false', 'true', 'true', 'false', ',', 'always')).toBe('"a","b"\n"z","y"\n');
  expect(await runWasm(page, 'id,note\n1,keep\n2,fix me\n3,keep\n', 'fix', 'fixed', 'note', 'regex', 'substring', 'false', 'false', 'false', 'true', 'true', 'false', ',', 'minimal', 'changed')).toBe('id,note\n2,fixed me\n');
  expect(await runWasm(page, 'a,b\nxx,x\nx,\n', 'x', 'y', '', 'regex', 'substring', 'false', 'false', 'false', 'true', 'true', 'false', ',', 'minimal', 'report')).toBe('column,cells_changed,replacements\na,2,3\nb,1,1\nTOTAL,3,4\n');

  await expect(runWasm(page, 'a\n1\n', '(', 'x')).rejects.toThrow(/invalid pattern/);
  await expect(runWasm(page, 'a,b\n1,2\n', '1', 'x', 'nope')).rejects.toThrow(/no column named 'nope'/);
});

test('csv-regex-replace enforces the advertised 5,000,000-byte cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/csv-regex-replace/gizza_ai_csv_regex_replace_web.js');
    await mod.default('/tools/csv-regex-replace/gizza_ai_csv_regex_replace_web_bg.wasm');
    const atCap = 'a\n' + 'x\n'.repeat(2_499_999);
    const overCap = atCap + 'x';
    const call = (data: string) => {
      try {
        return { ok: true, value: mod.run(data, 'x', 'y', '', 'regex', 'substring', 'false', 'false', 'false', 'true', 'true', 'false', 'auto', 'minimal', 'csv').slice(0, 5) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCapBytes).toBe(5_000_000);
  expect(result.overCapBytes).toBe(5_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('a\ny\ny');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/over the 5000000-byte limit/);
});

test('csv-regex-replace page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("Phone numbers")');
  await expect(page.locator('#in-columns')).toHaveValue('phone');
  await expect(page.locator('#in-pattern')).toHaveValue('[^0-9]');
});
