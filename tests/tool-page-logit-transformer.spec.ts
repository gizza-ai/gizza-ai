import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  data: string,
  direction = 'logit',
  base = 'e',
  separator = 'newline',
  outputSeparator = 'newline',
  onBoundary = 'fail',
  epsilon = '0.000001',
  decimals = '4',
  output = 'values',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/logit-transformer/gizza_ai_logit_transformer_web.js');
    await mod.default('/tools/logit-transformer/gizza_ai_logit_transformer_web_bg.wasm');
    return mod.run(
      args.data,
      args.direction,
      args.base,
      args.separator,
      args.outputSeparator,
      args.onBoundary,
      args.epsilon,
      args.decimals,
      args.output,
    );
  }, { data, direction, base, separator, outputSeparator, onBoundary, epsilon, decimals, output });
}

test('logit-transformer wasm computes logit anchors exactly', async ({ page }) => {
  await page.goto('/tools/logit-transformer/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, '0.5\n0.75\n0.25')).resolves.toBe(
    '0.0000\n1.0986\n-1.0986',
  );
});

test('logit-transformer wasm covers inverse, percent, clamp and table output', async ({ page }) => {
  await page.goto('/tools/logit-transformer/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, '0\n1\n-1', 'inverse')).resolves.toBe(
    '0.5000\n0.7311\n0.2689',
  );
  await expect(runWasm(page, '90%\n0.9', 'logit', 'e', 'newline', 'newline', 'fail', '0.000001', '6')).resolves.toBe(
    '2.197225\n2.197225',
  );
  await expect(runWasm(page, '0\n1', 'logit', 'e', 'newline', 'newline', 'clamp')).resolves.toBe(
    '-13.8155\n13.8155',
  );
  await expect(runWasm(page, '0.75', 'logit', 'e', 'newline', 'newline', 'fail', '0.000001', '4', 'table')).resolves.toBe(
    'probability\todds\tlogit\n0.75\t3.0000\t1.0986',
  );
});

test('logit-transformer page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/logit-transformer/');
  await setTextarea(page, '#in-data', '0.5\n0.75\n0.25');
  await page.locator('#in-decimals').fill('4');
  await expect(page.locator('#tool-output')).toHaveText('0.0000\n1.0986\n-1.0986', {
    timeout: 15_000,
  });
});

test('logit-transformer deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    data: '0\n1\n-1',
    direction: 'inverse',
    base: 'e',
    separator: 'newline',
    output_separator: 'newline',
    on_boundary: 'fail',
    epsilon: '0.000001',
    decimals: '4',
    output: 'values',
  });

  await page.goto(`/tools/logit-transformer/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(params.get('data')!, { timeout: 15_000 });
  await expect(page.locator('#in-direction')).toHaveValue('inverse');
  await expect(page.locator('#tool-output')).toHaveText('0.5000\n0.7311\n0.2689', {
    timeout: 15_000,
  });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool logit-transformer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
