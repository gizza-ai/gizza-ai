import { test, expect } from './fixtures';

const tool = '/tools/hdbscan-cluster/';
const sample = `x,y
1,1
1,2
2,1
2,2
1.5,1.5
1.2,1.8
10,10
10,11
11,10
11,11
10.5,10.5
10.2,10.8
60,-40`;

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    features: '',
    min_cluster_size: '5',
    min_samples: '0',
    metric: 'euclidean',
    alpha: '1.0',
    cluster_selection_epsilon: '0',
    selection: 'eom',
    allow_single_cluster: 'false',
    max_cluster_size: '0',
    normalize: 'true',
    missing: 'drop',
    sort: 'input',
    top: '0',
    only_noise: 'false',
    header: 'auto',
    delimiter: 'auto',
    decimals: '4',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/hdbscan-cluster/gizza_ai_hdbscan_cluster_web.js');
    await mod.default('/tools/hdbscan-cluster/gizza_ai_hdbscan_cluster_web_bg.wasm');
    return mod.run(
      args.input,
      args.features,
      args.min_cluster_size,
      args.min_samples,
      args.metric,
      args.alpha,
      args.cluster_selection_epsilon,
      args.selection,
      args.allow_single_cluster,
      args.max_cluster_size,
      args.normalize,
      args.missing,
      args.sort,
      args.top,
      args.only_noise,
      args.header,
      args.delimiter,
      args.decimals,
      args.format,
    );
  }, p);
}

test('hdbscan-cluster page renders clusters and noise', async ({ page }) => {
  await page.goto(tool);
  await setField(page, '#in-input', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('HDBSCAN — 2 clusters, 1 noise row', { timeout: 20_000 });
  await expect(output).toContainText('Rows clustered:   13 of 13 data rows');
  await expect(output).toContainText('Metric:           euclidean (standardized)');
  await expect(output).toContainText('60.0000  -40.0000');
});

test('hdbscan-cluster deep link prefills CSV output', async ({ page }) => {
  const qs = new URLSearchParams({
    input: sample,
    features: 'x,y',
    min_cluster_size: '5',
    min_samples: '0',
    metric: 'euclidean',
    alpha: '1.0',
    cluster_selection_epsilon: '0',
    selection: 'leaf',
    allow_single_cluster: 'false',
    max_cluster_size: '0',
    normalize: 'true',
    missing: 'drop',
    sort: 'cluster',
    top: '0',
    only_noise: 'false',
    header: 'auto',
    delimiter: 'auto',
    decimals: '3',
    format: 'csv',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-selection')).toHaveValue('leaf');
  await expect(page.locator('#in-format')).toHaveValue('csv');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('x,y,cluster,probability,outlier_score', { timeout: 20_000 });
  await expect(output).toContainText('60,-40,-1,0.000,');
});

test('hdbscan-cluster wasm covers enums, booleans, limits, delimiters, imputation, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const json = JSON.parse(await runWasm(page, {
    selection: 'leaf',
    metric: 'manhattan',
    sort: 'outlier',
    top: '3',
    format: 'json',
    decimals: '12',
  }));
  expect(json.metric).toBe('manhattan');
  expect(json.selection).toBe('leaf');
  expect(json.rows.length).toBe(3);

  const csv = await runWasm(page, {
    input: 'segment;value;cost\na;10;5\nb;11;6\nc;;5\nd;80;40\ne;9;5\nf;10;6\ng;12;5\nh;82;39',
    features: 'value,cost',
    min_cluster_size: '3',
    allow_single_cluster: 'true',
    missing: 'median',
    delimiter: 'semicolon',
    format: 'csv',
  });
  expect(csv).toContain('segment,value,cost,cluster,probability,outlier_score');
  expect(csv).toContain('d,80,40');

  const text = await runWasm(page, { metric: 'cosine', normalize: 'false', only_noise: 'true', top: '1' });
  expect(text).toContain('Metric:           cosine (raw values)');

  await expect(runWasm(page, { input: 'label\na\nb\nc\nd\ne\nf', features: '' })).rejects.toThrow(
    /no numeric columns/,
  );
});
