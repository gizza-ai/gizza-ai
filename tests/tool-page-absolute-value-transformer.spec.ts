import { test, expect } from './fixtures';

const INPUT = '-3\n4\n-2.5\n0';
const EXACT = '3\n4\n2.5\n0';

async function runWasm(
  page: any,
  data = INPUT,
  operation = 'abs',
  separator = 'auto',
  output_separator = 'same',
  decimals = 'auto',
  on_error = 'fail',
  output = 'values',
  stats = false,
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/absolute-value-transformer/gizza_ai_absolute_value_transformer_web.js');
    await mod.default('/tools/absolute-value-transformer/gizza_ai_absolute_value_transformer_web_bg.wasm');
    return mod.run(
      args.data,
      args.operation,
      args.separator,
      args.output_separator,
      args.decimals,
      args.on_error,
      args.output,
      args.stats ? 'true' : 'false',
    );
  }, { data, operation, separator, output_separator, decimals, on_error, output, stats });
}

test('absolute-value-transformer wasm applies abs sign negate and force-negative exactly', async ({ page }) => {
  await page.goto('/tools/absolute-value-transformer/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page)).toBe(EXACT);
  expect(await runWasm(page, '-7\n0\n0.001', 'sign', 'newline')).toBe('-1\n0\n1');
  expect(await runWasm(page, '-3,4,0', 'negate', 'comma', 'comma')).toBe('3,-4,0');
  expect(await runWasm(page, '3\n-4\n0', 'force-negative', 'newline')).toBe('-3\n-4\n0');
});

test('absolute-value-transformer page computes exact values from the form', async ({ page }) => {
  await page.goto('/tools/absolute-value-transformer/');
  await page.fill('#in-data', INPUT);
  await page.selectOption('#in-operation', 'abs');
  await page.selectOption('#in-separator', 'auto');
  await page.selectOption('#in-output_separator', 'same');
  await page.selectOption('#in-decimals', 'auto');
  await page.selectOption('#in-on_error', 'fail');
  await page.selectOption('#in-output', 'values');
  await page.uncheck('#in-stats');

  await expect(page.locator('#tool-output')).toHaveText(EXACT, { timeout: 15_000 });
});

test('absolute-value-transformer deep link covers rounding, stats, and non-default operation', async ({ page }) => {
  const params = new URLSearchParams({
    data: '1200.505\n340\n-75.25',
    operation: 'force-negative',
    separator: 'newline',
    output_separator: 'same',
    decimals: '2',
    on_error: 'fail',
    output: 'values',
    stats: 'true',
  });
  await page.goto(`/tools/absolute-value-transformer/?${params.toString()}`);

  await expect(page.locator('#in-operation')).toHaveValue('force-negative', { timeout: 15_000 });
  await expect(page.locator('#in-decimals')).toHaveValue('2');
  await expect(page.locator('#in-stats')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('-1200.51\n-340.00\n-75.25\n\n--- Summary ---\nCount: 3\nSum: -1615.76\nMin: -1200.51\nMax: -75.25\nMean: -538.59', { timeout: 15_000 });
});

test('absolute-value-transformer covers table/json/error modes, separator choices, cap boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/absolute-value-transformer/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, '-3\nn/a\n4', 'abs', 'newline', 'same', 'auto', 'blank', 'table'))
    .toBe('original\tresult\n-3\t3\nn/a\t\n4\t4');

  const json = await runWasm(page, '-3\nx', 'abs', 'newline', 'same', 'auto', 'blank', 'json');
  expect(json).toContain('"operation": "abs"');
  expect(json).toContain('"transformed": 1');
  expect(json).toContain('"invalid": 1');

  expect(await runWasm(page, '-1;-2', 'abs', 'semicolon', 'pipe')).toBe('1|2');
  await expect(runWasm(page, '$3', 'abs', 'newline')).rejects.toThrow(/strip currency symbols/);
  await expect(runWasm(page, '1/4', 'abs', 'newline')).rejects.toThrow(/fractions are not supported/);

  const atCap = Array(20000).fill('-1').join('\n');
  const atCapOut = await runWasm(page, atCap, 'abs', 'newline', 'newline', '0');
  expect(atCapOut.split('\n')).toHaveLength(20000);
  await expect(runWasm(page, `${atCap}\n-1`, 'abs', 'newline')).rejects.toThrow(/exceeds the maximum/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool absolute-value-transformer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
