import { test, expect } from './fixtures';

const tool = '/tools/npy-array-decoder/';
const sampleBase64 =
  'k05VTVBZAQB2AHsnZGVzY3InOiAnPGY4JywgJ2ZvcnRyYW5fb3JkZXInOiBGYWxzZSwgJ3NoYXBlJzogKDIsIDMpLCB9ICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAoAAAAAAADwPwAAAAAAAABAAAAAAAAADEAAAAAAAAAQQAAAAAAAABRAAAAAAAAAGEA=';
const sampleHex =
  '934e554d5059010076007b276465736372273a20277c7531272c2027666f727472616e5f6f72646572273a2046616c73652c20277368617065273a2028342c292c207d2020202020202020202020202020202020202020202020202020202020202020202020202020202020202020202020202020202020202020202020200a0a141e28';

async function runWasm(
  page,
  input: string,
  inputFormat = 'auto',
  output = 'summary',
  limit = '1000',
  delimiter = ',',
): Promise<string> {
  return await page.evaluate(
    async ({ input, inputFormat, output, limit, delimiter }) => {
      const mod = await import('/tools/npy-array-decoder/gizza_ai_npy_array_decoder_web.js');
      await mod.default('/tools/npy-array-decoder/gizza_ai_npy_array_decoder_web_bg.wasm');
      return mod.run(input, inputFormat, output, limit, delimiter);
    },
    { input, inputFormat, output, limit, delimiter },
  );
}

test('npy-array-decoder page renders a 2x3 float64 array as CSV', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', sampleBase64);
  await page.selectOption('#in-input_format', 'auto');
  await page.selectOption('#in-output', 'csv');
  await page.fill('#in-delimiter', ',');

  await expect(page.locator('#tool-output')).toHaveText('1,2,3.5\n4,5,6', { timeout: 15000 });
});

test('npy-array-decoder deep link pre-fills hex header-only state', async ({ page }) => {
  const qs = new URLSearchParams({
    input: sampleHex,
    input_format: 'hex',
    output: 'header',
    limit: '1000',
    delimiter: ',',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sampleHex, { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-output')).toHaveValue('header');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"dtype_name": "uint8"');
  await expect(out).toContainText('"shape": [4]');
  await expect(out).not.toContainText('"data":');
});

test('npy-array-decoder wasm covers output modes and validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, sampleBase64, 'auto', 'summary')).toContain('shape:    (2, 3)');
  expect(await runWasm(page, sampleBase64, 'base64', 'json')).toContain('"data": [[1, 2, 3.5], [4, 5, 6]]');
  expect(await runWasm(page, sampleBase64, 'base64', 'csv', '1000', 'tab')).toBe('1\t2\t3.5\n4\t5\t6');
  expect(await runWasm(page, sampleHex, 'hex', 'header')).toContain('"dtype_name": "uint8"');
  await expect(runWasm(page, 'not-a-npy-file', 'base64', 'summary')).rejects.toThrow(/not a \.npy file|invalid base64/);
  await expect(runWasm(page, sampleBase64, 'auto', 'yaml')).rejects.toThrow(/invalid output/);
});

test('npy-array-decoder page ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    '2x3 float64 array',
    'Same array as CSV',
    'uint8 vector from hex (header only)',
  ]);
});
