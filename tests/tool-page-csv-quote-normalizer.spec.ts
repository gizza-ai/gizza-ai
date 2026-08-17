import { test, expect } from './fixtures';

// Four different quoting conventions in one file: backslash escapes, curly
// "smart" quotes, padding before an opening quote, and a stray inner quote.
const sample =
  'id,name,note\n1,"Ada \\"Countess\\" Lovelace",fine\n2,“Grace Hopper”, "padded, quoted"\n3,"He said "hi" to me",ok';

async function runTool(
  page,
  params: {
    input?: string;
    delimiter?: string;
    output_delimiter?: string;
    input_quote?: string;
    quote_style?: string;
    output_quote?: string;
    escape?: string;
    backslash_escapes?: string;
    smart_quotes?: string;
    line_ending?: string;
    output?: string;
  } = {}
) {
  const p = {
    input: sample,
    delimiter: 'auto',
    output_delimiter: 'same',
    input_quote: 'auto',
    quote_style: 'minimal',
    output_quote: 'double',
    escape: 'doubled',
    backslash_escapes: 'true',
    smart_quotes: 'true',
    line_ending: 'lf',
    output: 'csv',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-quote-normalizer/gizza_ai_csv_quote_normalizer_web.js');
    await mod.default('/tools/csv-quote-normalizer/gizza_ai_csv_quote_normalizer_web_bg.wasm');
    return mod.run(
      args.input,
      args.delimiter,
      args.output_delimiter,
      args.input_quote,
      args.quote_style,
      args.output_quote,
      args.escape,
      args.backslash_escapes,
      args.smart_quotes,
      args.line_ending,
      args.output
    );
  }, p);
}

test('csv-quote-normalizer wasm returns exact default normalized CSV', async ({ page }) => {
  await page.goto('/tools/csv-quote-normalizer/');
  await page.waitForSelector('#in-input');

  const out = await runTool(page);
  expect(out).toBe(
    'id,name,note\n1,"Ada ""Countess"" Lovelace",fine\n2,Grace Hopper,"padded, quoted"\n3,"He said ""hi"" to me",ok\n'
  );
});

test('csv-quote-normalizer deep-link prefills controls and page output', async ({ page }) => {
  await page.goto(
    '/tools/csv-quote-normalizer/?delimiter=auto&output_delimiter=same&input_quote=auto&quote_style=always&output_quote=double&escape=backslash&backslash_escapes=true&smart_quotes=false&line_ending=lf&output=csv'
  );
  await page.waitForSelector('#in-input');
  await expect(page.locator('#in-delimiter')).toHaveValue('auto');
  await expect(page.locator('#in-output_delimiter')).toHaveValue('same');
  await expect(page.locator('#in-input_quote')).toHaveValue('auto');
  await expect(page.locator('#in-quote_style')).toHaveValue('always');
  await expect(page.locator('#in-output_quote')).toHaveValue('double');
  await expect(page.locator('#in-escape')).toHaveValue('backslash');
  await expect(page.locator('#in-line_ending')).toHaveValue('lf');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-backslash_escapes')).toBeChecked();
  await expect(page.locator('#in-smart_quotes')).not.toBeChecked();

  await page.fill('#in-input', 'a,b\n1,"x \\"y\\" z"\n');
  // quote_style = always + backslash escaping, and smart quotes off.
  await expect(page.locator('#tool-output')).toContainText('"x \\"y\\" z"', { timeout: 10_000 });
  await expect(page.locator('#tool-output')).toContainText('"a","b"');
});

test('csv-quote-normalizer covers advertised quote styles, dialects and the report', async ({
  page,
}) => {
  await page.goto('/tools/csv-quote-normalizer/');
  await page.waitForSelector('#in-input');

  const always = await runTool(page, { input: 'a,b\n1,2\n', quote_style: 'always' });
  expect(always).toBe('"a","b"\n"1","2"\n');

  const nonNumeric = await runTool(page, {
    input: 'a,b\n1,x\n-2.5e3,\n',
    quote_style: 'non_numeric',
  });
  expect(nonNumeric).toBe('"a","b"\n1,"x"\n-2.5e3,""\n');

  const never = await runTool(page, {
    input: 'a,b\n"x,y",2\n',
    quote_style: 'never',
    escape: 'backslash',
  });
  expect(never).toBe('a,b\nx\\,y,2\n');

  const single = await runTool(page, { input: 'a\n"x,y"\n', output_quote: 'single' });
  expect(single).toBe("a\n'x,y'\n");

  const toTsv = await runTool(page, { input: 'a;b\n"x;y";2\n', output_delimiter: 'tab' });
  expect(toTsv).toBe('a\tb\nx;y\t2\n');

  const crlf = await runTool(page, { input: 'a\n"one\ntwo"\n', line_ending: 'crlf' });
  expect(crlf).toBe('a\r\n"one\r\ntwo"\r\n');

  const literalCurly = await runTool(page, {
    input: 'a\n“x”\n',
    input_quote: 'double',
    smart_quotes: 'false',
  });
  expect(literalCurly).toBe('a\n“x”\n');

  const report = await runTool(page, { output: 'report' });
  expect(report).toContain('Input delimiter:  , (auto-detected)');
  expect(report).toContain('backslash-escaped quote');
  expect(report).toContain('curly quote');
  expect(report).toContain('padding before an opening quote removed');
  expect(report).toContain('stray quote inside a quoted field kept as literal text');
});

test('csv-quote-normalizer ships presets and reports validation errors', async ({ page }) => {
  await page.goto('/tools/csv-quote-normalizer/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("Backslash escaping for MySQL")');
  await expect(page.locator('#in-escape')).toHaveValue('backslash');
  await page.click('.tool-example-chip:has-text("See what changed (report)")');
  await expect(page.locator('#in-output')).toHaveValue('report');

  await expect(runTool(page, { quote_style: 'loose' })).rejects.toThrow(/quote_style must be/);
  await expect(
    runTool(page, { input: 'a,b\n"x,y",2\n', quote_style: 'never', escape: 'doubled' })
  ).rejects.toThrow(/row 2, field 1/);
  await expect(runTool(page, { input: '   ' })).rejects.toThrow(/empty/);
});
