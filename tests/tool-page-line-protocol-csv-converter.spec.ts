import { test, expect } from './fixtures';

const LP = 'cpu,host=host1,region=eu usage=64.23,busy=true 1577836800000000000';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('line-protocol-csv-converter converts line protocol to wide CSV', async ({ page }) => {
  await page.goto('/tools/line-protocol-csv-converter/');
  await page.fill('#in-data', LP);
  await page.selectOption('#in-direction', 'lp-to-csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('measurement,host,region,busy,usage,time', { timeout: 15000 });
  await expect(out).toContainText('cpu,host1,eu,true,64.23,2020-01-01T00:00:00Z');
});

test('line-protocol-csv-converter supports long CSV with Unix seconds', async ({ page }) => {
  await page.goto('/tools/line-protocol-csv-converter/');
  await page.fill('#in-data', LP);
  await page.selectOption('#in-direction', 'lp-to-csv');
  await page.selectOption('#in-csv_layout', 'long');
  await page.selectOption('#in-timestamp_format', 'unix_s');
  const text = await outputText(page);
  expect(text).toContain('measurement,host,region,field,value,time');
  expect(text).toContain('cpu,host1,eu,busy,true,1577836800');
  expect(text).toContain('cpu,host1,eu,usage,64.23,1577836800');
});

test('line-protocol-csv-converter converts CSV to line protocol with options', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'time,host,region,usage,count\n2020-01-01T00:00:00Z,host1,eu,64.23,7',
    direction: 'csv-to-lp',
    measurement: 'cpu',
    tag_columns: 'host,region',
    field_columns: 'usage,count',
    time_column: 'time',
    number_type: 'integer',
  });
  await page.goto(`/tools/line-protocol-csv-converter/?${params.toString()}`);
  await expect(page.locator('#tool-output')).toContainText('cpu,host=host1,region=eu', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toBe('cpu,host=host1,region=eu usage=64.23,count=7i 1577836800000000000');
});

test('line-protocol-csv-converter deep-link pre-fills and emits annotations', async ({ page }) => {
  const params = new URLSearchParams({
    data: LP,
    direction: 'lp-to-csv',
    emit_annotations: 'true',
  });
  await page.goto(`/tools/line-protocol-csv-converter/?${params.toString()}`);
  await expect(page.locator('#tool-output')).toContainText('#datatype measurement,tag,tag,boolean,double,dateTime:RFC3339', { timeout: 15000 });
});

test('line-protocol-csv-converter reports malformed line protocol', async ({ page }) => {
  await page.goto('/tools/line-protocol-csv-converter/');
  await page.fill('#in-data', 'cpu,host=a');
  await page.selectOption('#in-direction', 'lp-to-csv');
  await expect(page.locator('#tool-output')).toContainText('line 1', { timeout: 15000 });
});
