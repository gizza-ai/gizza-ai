import { test, expect } from './fixtures';

const tool = '/tools/mars-spline-regression/';
const sample = `x,y
0,5
1,4
2,3
3,2
4,1
5,0
6,1
7,2
8,3
9,4
10,5`;

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    data: sample,
    target: 'y',
    features: 'x',
    max_terms: '21',
    max_degree: '1',
    penalty: '3',
    prune: 'true',
    nprune: '0',
    minspan: '0',
    endspan: '0',
    thresh: '0.001',
    allow_linear: 'true',
    predict: '11\n12',
    header: 'auto',
    decimals: '4',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/mars-spline-regression/gizza_ai_mars_spline_regression_web.js');
    await mod.default('/tools/mars-spline-regression/gizza_ai_mars_spline_regression_web_bg.wasm');
    return mod.run(
      args.data,
      args.target,
      args.features,
      args.max_terms,
      args.max_degree,
      args.penalty,
      args.prune,
      args.nprune,
      args.minspan,
      args.endspan,
      args.thresh,
      args.allow_linear,
      args.predict,
      args.header,
      args.decimals,
      args.format,
    );
  }, p);
}

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('mars-spline-regression page renders a hinge equation and predictions', async ({ page }) => {
  await page.goto(tool);
  await setField(page, '#in-data', sample);
  await setField(page, '#in-target', 'y');
  await setField(page, '#in-features', 'x');
  await setField(page, '#in-predict', '11\n12');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('y = 0.0000', { timeout: 20_000 });
  await expect(output).toContainText('h(x - 5.0000)');
  await expect(output).toContainText('R²                1.0000');
  await expect(output).toContainText('x=11.0000  ->  y = 6.0000');
});

test('mars-spline-regression deep link can prefill CSV output', async ({ page }) => {
  const qs = new URLSearchParams({
    data: sample,
    target: 'y',
    features: 'x',
    max_terms: '15',
    max_degree: '1',
    penalty: '3',
    prune: 'true',
    nprune: '0',
    minspan: '0',
    endspan: '0',
    thresh: '0.001',
    allow_linear: 'true',
    predict: '',
    header: 'auto',
    decimals: '3',
    format: 'csv',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-target')).toHaveValue('y');
  await expect(page.locator('#in-format')).toHaveValue('csv');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('term,coefficient,degree', { timeout: 20_000 });
  await expect(output).toContainText('h(x - 5.000),1.000,1');
});

test('mars-spline-regression wasm covers formats, enum values, checkboxes, and bounds', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const json = JSON.parse(await runWasm(page, { format: 'json', decimals: '12', max_degree: '3' }));
  expect(json.kept_terms).toBe(3);
  expect(json.knots[0].knots[0]).toBe(5);
  expect(json.features).toEqual(['x']);

  const csv = await runWasm(page, { format: 'csv', header: 'yes', decimals: '3' });
  expect(csv).toContain('term,coefficient,degree');
  expect(csv).toContain('h(5.000 - x),1.000,1');

  const noPrune = await runWasm(page, { prune: 'false', format: 'text' });
  expect(noPrune).toContain('from the forward pass (pruning off)');

  await expect(runWasm(page, { data: 'x,y\n1,2\n2,2\n3,2', format: 'text' })).rejects.toThrow(
    /constant/,
  );
});
