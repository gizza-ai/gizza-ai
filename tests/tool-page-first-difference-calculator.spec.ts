import { test, expect } from './fixtures';

const tool = '/tools/first-difference-calculator/';

async function runWasm(
  page,
  series: string,
  lag = '1',
  order = '1',
  mode = 'difference',
  decimals = '6',
  dropWarmup = 'false',
): Promise<string> {
  return await page.evaluate(
    async ({ series, lag, order, mode, decimals, dropWarmup }) => {
      const mod = await import('/tools/first-difference-calculator/gizza_ai_first_difference_calculator_web.js');
      await mod.default('/tools/first-difference-calculator/gizza_ai_first_difference_calculator_web_bg.wasm');
      return mod.run(series, lag, order, mode, decimals, dropWarmup);
    },
    { series, lag, order, mode, decimals, dropWarmup },
  );
}

test('first-difference-calculator page renders exact aligned differences', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-series', '2, 5, 9, 14');
  await page.fill('#in-lag', '1');
  await page.fill('#in-order', '1');
  await page.selectOption('#in-mode', 'difference');
  await page.fill('#in-decimals', '6');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"values": [', { timeout: 15_000 });
  await expect(out).toContainText('null,');
  await expect(out).toContainText('3,');
  await expect(out).toContainText('4,');
  await expect(out).toContainText('5');
  await expect(out).toContainText('"largest_move_index": 3');
});

test('first-difference-calculator deep link pre-fills percent shorter output', async ({ page }) => {
  const qs = new URLSearchParams({
    series: '100, 110, 99',
    lag: '1',
    order: '1',
    mode: 'percent',
    decimals: '2',
    drop_warmup: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-series')).toHaveValue('100, 110, 99', { timeout: 15_000 });
  await expect(page.locator('#in-lag')).toHaveValue('1');
  await expect(page.locator('#in-order')).toHaveValue('1');
  await expect(page.locator('#in-mode')).toHaveValue('percent');
  await expect(page.locator('#in-decimals')).toHaveValue('2');
  await expect(page.locator('#in-drop_warmup')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"values": [');
  await expect(out).toContainText('10.0,');
  await expect(out).toContainText('-10.0');
  await expect(out).toContainText('"mode": "percent"');
  await expect(out).toContainText('"drop_warmup": true');
});

test('first-difference-calculator wasm covers modes, lag forms, boundaries, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-series');

  const second = JSON.parse(await runWasm(page, '1, 4, 9, 16, 25', '1', '2'));
  expect(second.values).toEqual([null, null, 2, 2, 2]);
  expect(second.summary.constant).toBe(true);
  expect(second.interpretation).toContain('quadratic relation');

  const lead = JSON.parse(await runWasm(page, '2, 5, 9, 14', '-1', '1', 'difference', '6', 'true'));
  expect(lead.values).toEqual([-3, -4, -5]);
  expect(lead.indices).toEqual([0, 1, 2]);

  const ratio = JSON.parse(await runWasm(page, '4, 8, 8, 2', '1', '1', 'ratio', '4', 'true'));
  expect(ratio.values).toEqual([2, 1, 0.25]);
  expect(ratio.summary).toMatchObject({ increases: 1, unchanged: 1, decreases: 1 });

  const log = JSON.parse(await runWasm(page, '1, 2.718281828459045', '1', '1', 'log', '6', 'true'));
  expect(log.values).toEqual([1]);

  const zero = JSON.parse(await runWasm(page, '0, 5, 10', '1', '1', 'percent', '6', 'true'));
  expect(zero.values).toEqual([null, 100]);
  expect(zero.summary.undefined).toBe(1);

  const maxDecimals = JSON.parse(await runWasm(page, '3, 10', '1', '1', 'percent', '10', 'true'));
  expect(maxDecimals.values[0]).toBeCloseTo(233.3333333333, 10);

  await expect(runWasm(page, '1, 2, 3', '0')).rejects.toThrow(/lag must not be 0/);
  await expect(runWasm(page, '1, 2, 3', '1', '11')).rejects.toThrow(/order must be between 1 and 10/);
});

test('first-difference-calculator ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Monthly deltas',
    'Percent change',
    'Seasonal lag 12',
    'Second differences',
    'Log growth rate',
  ]);
});
