import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  data = 'Kittens: 4.2 5.1 4.8 5.6 5.1\nOxen: 792 800 803 795 797',
  basis = 'sample',
  grouping = 'auto',
  delimiter = 'auto',
  mean = '',
  std_dev = '',
  exclude_outliers = false,
  ignore_non_numeric = false,
  decimals = '4',
  output = 'summary',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/coefficient-of-variation-calculator/gizza_ai_coefficient_of_variation_calculator_web.js');
    await mod.default('/tools/coefficient-of-variation-calculator/gizza_ai_coefficient_of_variation_calculator_web_bg.wasm');
    return mod.run(
      args.data,
      args.basis,
      args.grouping,
      args.delimiter,
      args.mean,
      args.std_dev,
      args.exclude_outliers,
      args.ignore_non_numeric,
      args.decimals,
      args.output,
    );
  }, { data, basis, grouping, delimiter, mean, std_dev, exclude_outliers, ignore_non_numeric, decimals, output });
}

test('coefficient-of-variation page ranks datasets and exposes CLI example', async ({ page }) => {
  await page.goto('/tools/coefficient-of-variation-calculator/');
  await page.fill('#in-data', 'Kittens: 4.2 5.1 4.8 5.6 5.1\nOxen: 792 800 803 795 797');
  await page.selectOption('#in-basis', 'sample');
  await page.selectOption('#in-grouping', 'auto');
  await page.selectOption('#in-delimiter', 'auto');
  await page.fill('#in-decimals', '4');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Rank 1 — Oxen', { timeout: 15_000 });
  await expect(out).toContainText('Most consistent  : Oxen');
  await expect(out).toContainText('Spread ratio');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool coefficient-of-variation-calculator');
  expect(cli).toContain('Kittens: 4.2 5.1');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('coefficient-of-variation deep-link prefills summary mode', async ({ page }) => {
  const params = new URLSearchParams({
    data: '',
    basis: 'sample',
    grouping: 'auto',
    delimiter: 'auto',
    mean: '23.41',
    std_dev: '0.783',
    exclude_outliers: 'false',
    ignore_non_numeric: 'false',
    decimals: '4',
    output: 'summary',
  });
  await page.goto(`/tools/coefficient-of-variation-calculator/?${params.toString()}`);
  await expect(page.locator('#in-mean')).toHaveValue('23.41', { timeout: 15_000 });
  await expect(page.locator('#in-std_dev')).toHaveValue('0.783');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('CV               = 0.0334', { timeout: 15_000 });
  await expect(out).toContainText('CV %             = 3.3447%');
});

test('coefficient-of-variation wasm covers advertised controls and errors', async ({ page }) => {
  await page.goto('/tools/coefficient-of-variation-calculator/');
  await page.waitForSelector('#in-data');

  const table = await runWasm(page, '2\n4\n4\n4\n5\n5\n7\n9', 'population', 'auto', 'auto', '', '', false, false, '2', 'table');
  expect(table).toContain('| 1 | Dataset 1 | 8 | 5.00 | 2.00 | 0.40 | 40.00% | very high |');

  const json = await runWasm(page, 'Machine A: 10, 11, n/a, 10, 12, 11, 10, 11, 90', 'sample', 'auto', 'comma', '', '', true, true, '3', 'json');
  expect(json).toContain('"outliers_removed": [\n        90.0\n      ]');
  expect(json).toContain('"relative_spread"');

  const summary = await runWasm(page, '', 'sample', 'auto', 'auto', '23.41', '0.783', false, false, '4', 'summary');
  expect(summary).toContain('CV               = 0.0334');

  await expect(runWasm(page, '', 'sample')).rejects.toThrow(/provide data/);
});
