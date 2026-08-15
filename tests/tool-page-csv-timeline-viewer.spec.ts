import { test, expect, type Page } from './fixtures';

const tool = '/tools/csv-timeline-viewer/';
const CSV_DATA =
  'timestamp,level,service,message\n' +
  '2024-06-01T10:00:01Z,INFO,api,request started\n' +
  '2024-06-01T10:00:05Z,ERROR,api,upstream timeout\n' +
  '2024-06-01T10:00:09Z,WARN,worker,retrying job 42\n' +
  '2024-06-02T11:30:00Z,ERROR,worker,job 42 failed';
const JSONL_DATA =
  '{"ts":"2024-06-01T10:00:00Z","event":"login","user":"ada"}\n' +
  '{"ts":"2024-06-01T10:04:00Z","event":"login","user":"bo"}\n' +
  '{"ts":"2024-06-01T10:05:00Z","event":"logout","user":"ada"}';

async function outputText(page: Page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('csv-timeline-viewer filters a CSV timeline and renders a real table', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', CSV_DATA);
  await page.fill('#in-filters', 'level == ERROR');

  await expect(page.locator('#tool-output')).toContainText('upstream timeout', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('job 42 failed');
  expect(text).toContain('showing rows 1-2 of 2 matched (4 read)');
  expect(text).toContain('time column: timestamp');
  expect(text).not.toContain('request started');
});

test('csv-timeline-viewer supports date ranges, projection, and descending sort', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', CSV_DATA);
  await page.fill('#in-from', '2024-06-01');
  await page.fill('#in-to', '2024-06-01');
  await page.selectOption('#in-order', 'desc');
  await page.fill('#in-columns', 'timestamp, message');
  await page.fill('#in-limit', '2');

  await expect(page.locator('#tool-output')).toContainText('retrying job 42', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('upstream timeout');
  expect(text).not.toContain('service');
  expect(text.indexOf('retrying job 42')).toBeLessThan(text.indexOf('upstream timeout'));
  expect(text).toContain('showing rows 1-2 of 3 matched (4 read)');
});

test('csv-timeline-viewer emits JSON Lines summaries from JSONL input', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', JSONL_DATA);
  await page.selectOption('#in-format', 'jsonl');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toContainText('Rows read: 3', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('Time column:    ts');
  expect(text).toContain('Earliest:       2024-06-01T10:00:00Z');
  expect(text).toContain('Latest:         2024-06-01T10:05:00Z');
  expect(text).toContain('Bucket:         10 seconds');
});

test('csv-timeline-viewer regex search, field restriction, and non-default checkbox states work', async ({
  page,
}) => {
  await page.goto(tool);
  await page.fill('#in-data', CSV_DATA);
  await page.fill('#in-search', 'job \\d+');
  await page.fill('#in-search_fields', 'message');
  await page.check('#in-regex');
  await page.check('#in-case_sensitive');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toContainText('retrying job 42', { timeout: 15_000 });
  const rows = JSON.parse(await outputText(page)) as Array<Record<string, string>>;
  expect(rows).toHaveLength(2);
  expect(rows.map((r) => r.level)).toEqual(['WARN', 'ERROR']);
});

test('csv-timeline-viewer deep link prefills and auto-runs a CSV output', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent(CSV_DATA) +
      '&format=csv&delimiter=comma&header=true&time_column=timestamp&from=2024-06-01&to=2024-06-01&tz_offset=0&search=timeout&search_fields=message&regex=false&case_sensitive=false&filters=&sort_by=timestamp&order=asc&columns=timestamp,message&limit=10&offset=0&output=csv',
  );

  await expect(page.locator('#in-data')).toHaveValue(CSV_DATA, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-time_column')).toHaveValue('timestamp');
  await expect(page.locator('#in-search')).toHaveValue('timeout');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('upstream timeout', { timeout: 15_000 });
  expect(await outputText(page)).toBe('timestamp,message\n2024-06-01T10:00:05Z,upstream timeout');
});

test('csv-timeline-viewer headerless pipe data uses generated column names', async ({ page }) => {
  await page.goto(tool);
  await page.fill(
    '#in-data',
    '2024-06-01T10:00:01Z|INFO|api|request started\n2024-06-01T10:00:05Z|ERROR|api|upstream timeout',
  );
  await page.selectOption('#in-delimiter', 'pipe');
  await page.uncheck('#in-header');
  await page.fill('#in-time_column', 'column1');
  await page.fill('#in-filters', 'column2 == ERROR');

  await expect(page.locator('#tool-output')).toContainText('column4', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('upstream timeout');
  expect(text).not.toContain('request started');
});

test('csv-timeline-viewer reports bad ranges without hiding the expected input', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', CSV_DATA);
  await page.fill('#in-from', '2024-06-03');
  await page.fill('#in-to', '2024-06-01');

  await expect(page.locator('#tool-output')).toContainText('from is later than to', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('swap the range bounds');
});
