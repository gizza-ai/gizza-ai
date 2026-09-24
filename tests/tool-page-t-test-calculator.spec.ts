import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  data = '1,4\n2,5\n3,6\n4,7\n5,8',
  kind = 'welch',
  format = 'wide',
  delimiter = 'comma',
  header = 'no',
  mu = '0',
  tails = 'two',
  alpha = '0.05',
  decimals = '4',
  output = 'summary',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/t-test-calculator/gizza_ai_t_test_calculator_web.js');
    await mod.default('/tools/t-test-calculator/gizza_ai_t_test_calculator_web_bg.wasm');
    return mod.run(
      args.data,
      args.kind,
      args.format,
      args.delimiter,
      args.header,
      args.mu,
      args.tails,
      args.alpha,
      args.decimals,
      args.output,
    );
  }, { data, kind, format, delimiter, header, mu, tails, alpha, decimals, output });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('t-test-calculator wasm covers advertised t-test modes and outputs', async ({ page }) => {
  await page.goto('/tools/t-test-calculator/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, '5\n7\n9\n11\n13', 'one-sample', 'wide', 'auto', 'no', '8')).resolves.toContain('One-sample t-test');
  await expect(runWasm(page, '1,4\n2,5\n3,6\n4,7\n5,8', 'two-sample')).resolves.toContain('Two-sample (pooled) t-test');
  await expect(runWasm(page, '1,4\n2,5\n3,6\n4,7\n5,8', 'welch')).resolves.toContain("Welch's unequal-variance t-test");
  await expect(runWasm(page, 'before,after\n12,10\n14,11\n11,10\n15,12\n13,12\n16,13', 'paired', 'wide', 'comma', 'yes')).resolves.toContain('Paired-samples t-test');

  const json = await runWasm(page, '1,4\n2,5\n3,6\n4,7\n5,8', 'welch', 'wide', 'comma', 'no', '0', 'right', '0.5', '3', 'json');
  expect(json).toContain('"test": "welch"');
  expect(json).toContain('"tails": "right"');
  expect(json).toContain('"alpha": 0.5');
});

test('t-test-calculator page renders real paired output and CLI example', async ({ page }) => {
  await page.goto('/tools/t-test-calculator/');
  await page.fill('#in-data', 'before,after\n12,10\n14,11\n11,10\n15,12\n13,12\n16,13');
  await page.selectOption('#in-test', 'paired');
  await page.selectOption('#in-format', 'wide');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-header', 'yes');
  await page.fill('#in-mu', '0');
  await page.selectOption('#in-tails', 'two');
  await page.fill('#in-alpha', '0.05');
  await page.fill('#in-decimals', '4');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toContainText('Paired-samples t-test', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('t(5)', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText("Cohen's d (dz)");

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool t-test-calculator');
  expect(cli).toContain('12,10');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('t-test-calculator deep-link pre-fills and returns JSON', async ({ page }) => {
  const params = new URLSearchParams({
    data: '1,4\n2,5\n3,6\n4,7\n5,8',
    test: 'welch',
    format: 'wide',
    delimiter: 'comma',
    header: 'no',
    mu: '0',
    tails: 'left',
    alpha: '0.01',
    decimals: '4',
    output: 'json',
  });

  await page.goto(`/tools/t-test-calculator/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue('1,4\n2,5\n3,6\n4,7\n5,8', { timeout: 15_000 });
  await expect(page.locator('#in-test')).toHaveValue('welch');
  await expect(page.locator('#in-tails')).toHaveValue('left');
  await expect(page.locator('#in-alpha')).toHaveValue('0.01');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"test": "welch"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"tails": "left"');
  expect(await outputText(page)).toContain('"p_value"');
});
