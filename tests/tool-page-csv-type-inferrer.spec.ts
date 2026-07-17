import { test, expect } from './fixtures';

// /tools/csv-type-inferrer/ sniffs a CSV's dialect and infers per-column types
// entirely in-browser (pure wasm) — nothing is uploaded.
const SAMPLE = 'id,name,active,joined,score\n1,Alice,true,2021-05-01,9.5\n2,Bob,false,2022-11-30,7';

test('csv-type-inferrer infers dialect and per-column types', async ({ page }) => {
  await page.goto('/tools/csv-type-inferrer/');
  await page.fill('#in-data', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"delimiter_name": "comma"', { timeout: 15000 });

  const report = JSON.parse((await out.textContent()) ?? '{}');
  expect(report.dialect.delimiter).toBe(',');
  expect(report.dialect.has_header).toBe(true);
  expect(report.row_count).toBe(2);
  expect(report.column_count).toBe(5);
  const types = report.columns.map((c: { type: string }) => c.type);
  expect(types).toEqual(['int', 'string', 'bool', 'date', 'float']);
  expect(report.columns[3].format).toBe('YYYY-MM-DD');
  // typed records ride along with the default "both" output
  expect(report.records[0].id).toBe(1);
  expect(report.records[0].active).toBe(true);
  expect(report.records[1].score).toBe(7);
});

test('csv-type-inferrer honors the output enum via the select', async ({ page }) => {
  await page.goto('/tools/csv-type-inferrer/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'schema');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"column_count": 5', { timeout: 15000 });
  const report = JSON.parse((await out.textContent()) ?? '{}');
  // "schema" drops the records array entirely
  expect(report.records).toBeUndefined();
  expect(report.columns).toHaveLength(5);
});

test('csv-type-inferrer sniffs a semicolon table with no header', async ({ page }) => {
  await page.goto('/tools/csv-type-inferrer/');
  await page.fill('#in-data', '10;20;30\n40;50;60');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"delimiter_name": "semicolon"', { timeout: 15000 });
  const report = JSON.parse((await out.textContent()) ?? '{}');
  expect(report.dialect.has_header).toBe(false);
  expect(report.columns[0].name).toBe('column_1');
  expect(report.columns[0].type).toBe('int');
});

test('csv-type-inferrer honors a query-param deep link (records output)', async ({ page }) => {
  const params = new URLSearchParams({ data: SAMPLE, delimiter: 'comma', output: 'records' });
  await page.goto(`/tools/csv-type-inferrer/?${params.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('records');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"Alice"', { timeout: 15000 });
  const records = JSON.parse((await out.textContent()) ?? '[]');
  expect(Array.isArray(records)).toBe(true);
  expect(records).toHaveLength(2);
  expect(records[0]).toEqual({
    id: 1,
    name: 'Alice',
    active: true,
    joined: '2021-05-01',
    score: 9.5,
  });
});
