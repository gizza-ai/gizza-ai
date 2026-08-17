import { test, expect } from './fixtures';

const tool = '/tools/percent-difference-calculator/';

async function runWasm(
  page,
  a: string,
  b: string,
  mode = 'all',
  decimals = '4',
): Promise<string> {
  return await page.evaluate(
    async ({ a, b, mode, decimals }) => {
      const mod = await import('/tools/percent-difference-calculator/gizza_ai_percent_difference_calculator_web.js');
      await mod.default('/tools/percent-difference-calculator/gizza_ai_percent_difference_calculator_web_bg.wasm');
      return mod.run(a, b, mode, decimals);
    },
    { a, b, mode, decimals },
  );
}

test('percent-difference-calculator page renders the worked example', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-a', '70');
  await page.fill('#in-b', '85');
  await page.selectOption('#in-mode', 'all');
  await page.fill('#in-decimals', '4');

  await expect(page.locator('#tool-output')).toHaveText(
    'Comparing a = 70 and b = 85\n\n' +
      'Absolute difference |a - b| = 15\n' +
      'Signed difference b - a = 15 (increase)\n' +
      'Mean (a + b) / 2 = 77.5\n\n' +
      'Percent difference = 19.3548%   (|a - b| / |mean| * 100)\n\n' +
      'Percent change a -> b = 21.4286%   ((b - a) / |a| * 100)\n' +
      'Percent change b -> a = -17.6471%\n' +
      'Ratio b / a = 1.2143',
    { timeout: 15000 },
  );
});

test('percent-difference-calculator deep link pre-fills difference-only state', async ({ page }) => {
  const qs = new URLSearchParams({
    a: '5',
    b: '7',
    mode: 'difference',
    decimals: '2',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-a')).toHaveValue('5', { timeout: 15000 });
  await expect(page.locator('#in-b')).toHaveValue('7');
  await expect(page.locator('#in-mode')).toHaveValue('difference');
  await expect(page.locator('#in-decimals')).toHaveValue('2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Percent difference = 33.33%');
  await expect(out).not.toContainText('Percent change');
});

test('percent-difference-calculator wasm covers modes and decimal boundaries', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-a');

  expect(await runWasm(page, '70', '85', 'all', '4')).toContain('Percent change a -> b = 21.4286%');
  expect(await runWasm(page, '5', '7', 'difference', '2')).toContain('Percent difference = 33.33%');
  expect(await runWasm(page, '120', '100', 'change', '2')).toContain('Percent change a -> b = -16.67%');
  expect(await runWasm(page, '10', '6', 'all', '0')).toContain('Percent difference = 50%');
  expect(await runWasm(page, '1', '3', 'all', '10')).toContain('Percent difference = 100%');
  await expect(runWasm(page, '1', '2', 'all', '11')).rejects.toThrow(/decimals must be between 0 and 10/);
});

test('percent-difference-calculator page ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    '70 vs 85',
    '5 vs 7 (difference only)',
    '120 → 100 (change only)',
    'Negative pair: -4 vs 6',
    'Whole percents',
  ]);
});
