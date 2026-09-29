import { test, expect } from './fixtures';

const tool = '/tools/isolation-forest-anomaly/';
const sample = `temp,pressure
20,101
21,102
20,100
22,101
21,101
20,102
21,100
90,300`;

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
    method: 'standard',
    trees: '100',
    sample_size: 'auto',
    max_features: '1',
    bootstrap: 'false',
    contamination: 'auto',
    threshold: '0',
    missing: 'drop',
    sort: 'score',
    top: '0',
    only_anomalies: 'false',
    header: 'auto',
    delimiter: 'auto',
    decimals: '4',
    seed: '42',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/isolation-forest-anomaly/gizza_ai_isolation_forest_anomaly_web.js');
    await mod.default('/tools/isolation-forest-anomaly/gizza_ai_isolation_forest_anomaly_web_bg.wasm');
    return mod.run(
      args.input,
      args.features,
      args.method,
      args.trees,
      args.sample_size,
      args.max_features,
      args.bootstrap,
      args.contamination,
      args.threshold,
      args.missing,
      args.sort,
      args.top,
      args.only_anomalies,
      args.header,
      args.delimiter,
      args.decimals,
      args.seed,
      args.format,
    );
  }, p);
}

test('isolation-forest-anomaly page renders an outlier report', async ({ page }) => {
  await page.goto(tool);
  await setField(page, '#in-input', sample);
  await page.selectOption('#in-sort', 'score');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Isolation Forest — standard', { timeout: 20_000 });
  await expect(output).toContainText('Rows scored:      8 of 8 data rows');
  await expect(output).toContainText('Anomalies:        1 of 8 rows');
  await expect(output).toContainText('90      300');
});

test('isolation-forest-anomaly deep link prefills JSON extended output', async ({ page }) => {
  const qs = new URLSearchParams({
    input: sample,
    features: 'temp,pressure',
    method: 'extended',
    trees: '80',
    sample_size: 'auto',
    max_features: '1',
    bootstrap: 'false',
    contamination: '10%',
    threshold: '0',
    missing: 'drop',
    sort: 'score',
    top: '3',
    only_anomalies: 'true',
    header: 'auto',
    delimiter: 'auto',
    decimals: '4',
    seed: '7',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-method')).toHaveValue('extended');
  await expect(page.locator('#in-format')).toHaveValue('json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"method": "extended"', { timeout: 20_000 });
  await expect(output).toContainText('"features": ["temp", "pressure"]');
  await expect(output).toContainText('"anomalies": 1');
});

test('isolation-forest-anomaly wasm covers formats, enums, booleans, imputation, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const csv = await runWasm(page, {
    input: 'segment,value,cost\na,10,5\nb,11,6\nc,,5\nd,80,40\ne,9,5',
    features: 'value,cost',
    missing: 'median',
    contamination: '20%',
    format: 'csv',
    decimals: '3',
  });
  expect(csv).toContain('segment,value,cost,anomaly_score,path_length,is_anomaly,rank');
  expect(csv).toContain('d,80,40');

  const json = JSON.parse(await runWasm(page, {
    method: 'extended',
    bootstrap: 'true',
    sample_size: '50%',
    max_features: '1',
    threshold: '0.5',
    only_anomalies: 'true',
    format: 'json',
    seed: '9',
  }));
  expect(json.method).toBe('extended');
  expect(json.bootstrap).toBe(true);
  expect(json.rows.length).toBeGreaterThanOrEqual(1);

  const text = await runWasm(page, { delimiter: 'semicolon', input: 'x;y\n1;1\n2;1\n1;2\n50;50' });
  expect(text).toContain('Rows scored:      4 of 4 data rows');

  await expect(runWasm(page, { input: 'label\na\nb', features: '', format: 'text' })).rejects.toThrow(
    /no numeric columns/,
  );
});
