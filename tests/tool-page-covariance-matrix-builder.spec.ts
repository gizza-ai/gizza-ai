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
  labels = '',
  delimiter = 'auto',
  header = 'auto',
  matrix = 'covariance',
  denominator = 'sample',
  weights = '',
  decimals = '6',
  stats = 'false',
  format = 'csv',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/covariance-matrix-builder/gizza_ai_covariance_matrix_builder_web.js');
    await mod.default('/tools/covariance-matrix-builder/gizza_ai_covariance_matrix_builder_web_bg.wasm');
    return mod.run(
      args.data,
      args.labels,
      args.delimiter,
      args.header,
      args.matrix,
      args.denominator,
      args.weights,
      args.decimals,
      args.stats,
      args.format,
    );
  }, { data, labels, delimiter, header, matrix, denominator, weights, decimals, stats, format });
}

test('covariance-matrix-builder wasm computes sample covariance exactly', async ({ page }) => {
  await page.goto('/tools/covariance-matrix-builder/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, 'x,y\n1,2\n2,4\n3,5')).resolves.toBe(
    ',x,y\nx,1.000000,1.500000\ny,1.500000,2.333333',
  );
});

test('covariance-matrix-builder wasm covers population, correlation, centered and weights', async ({ page }) => {
  await page.goto('/tools/covariance-matrix-builder/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, 'x,y\n1,2\n2,4\n3,5', '', 'auto', 'auto', 'covariance', 'population')).resolves.toBe(
    ',x,y\nx,0.666667,1.000000\ny,1.000000,1.555556',
  );
  await expect(runWasm(page, 'x,y\n1,2\n2,4\n3,5', '', 'auto', 'auto', 'correlation', 'sample', '', '4')).resolves.toBe(
    ',x,y\nx,1.0000,0.9820\ny,0.9820,1.0000',
  );
  await expect(runWasm(page, 'x,y\n1,2\n2,4\n3,5', '', 'auto', 'auto', 'centered', 'sample', '', '1')).resolves.toBe(
    'row,x,y\n1,-1.0,-1.7\n2,0.0,0.3\n3,1.0,1.3',
  );
  await expect(runWasm(page, 'x,y\n1,2\n2,4\n3,5', '', 'auto', 'auto', 'covariance', 'sample', '2,1,1')).resolves.toContain(
    'x,0.916667,1.416667',
  );
});

test('covariance-matrix-builder page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/covariance-matrix-builder/');
  await setTextarea(page, '#in-data', 'x,y\n1,2\n2,4\n3,5');
  await page.locator('#in-format').selectOption('csv');
  await page.locator('#in-stats').setChecked(false);
  await expect(page.locator('#tool-output')).toHaveText(',x,y\nx,1.000000,1.500000\ny,1.500000,2.333333', {
    timeout: 15_000,
  });
});

test('covariance-matrix-builder deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'x,y\n1,2\n2,4\n3,5',
    matrix: 'correlation',
    denominator: 'sample',
    decimals: '4',
    stats: 'false',
    format: 'csv',
  });

  await page.goto(`/tools/covariance-matrix-builder/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(params.get('data')!, { timeout: 15_000 });
  await expect(page.locator('#in-matrix')).toHaveValue('correlation');
  await expect(page.locator('#tool-output')).toHaveText(',x,y\nx,1.0000,0.9820\ny,0.9820,1.0000', {
    timeout: 15_000,
  });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool covariance-matrix-builder');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
