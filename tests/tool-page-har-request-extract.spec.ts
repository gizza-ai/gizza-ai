import { test, expect } from './fixtures';

// Three-request capture: an HTML page (bodySize), a slow API call (only
// Chrome's _transferSize known), and a 404 image.
const har = JSON.stringify({
  log: {
    entries: [
      {
        startedDateTime: '2024-01-01T00:00:00.000Z',
        time: 102.5,
        request: { method: 'GET', url: 'https://example.com/' },
        response: {
          status: 200,
          statusText: 'OK',
          content: { mimeType: 'text/html; charset=utf-8', size: 5120 },
          bodySize: 2048,
        },
      },
      {
        startedDateTime: '2024-01-01T00:00:01.000Z',
        time: 812,
        request: { method: 'POST', url: 'https://example.com/api/search' },
        response: {
          status: 200,
          statusText: 'OK',
          content: { mimeType: 'application/json', size: 20480 },
          bodySize: -1,
          _transferSize: 10240,
        },
      },
      {
        startedDateTime: '2024-01-01T00:00:02.000Z',
        time: 54,
        request: { method: 'GET', url: 'https://cdn.example.com/logo.png' },
        response: {
          status: 404,
          statusText: 'Not Found',
          content: { mimeType: 'image/png', size: 0 },
          bodySize: 512,
        },
      },
    ],
  },
});

const fullTable = [
  '3 of 3 requests · 12.5 KB transferred',
  '',
  '#  METHOD  STATUS  TYPE              SIZE     TIME    URL',
  '1  GET     200     text/html         2.0 KB   103 ms  https://example.com/',
  '2  POST    200     application/json  10.0 KB  812 ms  https://example.com/api/search',
  '3  GET     404     image/png         512 B    54 ms   https://cdn.example.com/logo.png',
].join('\n');

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('default table lists every request with an exact summary + columns', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  await expect(page.locator('#tool-output')).toContainText('3 of 3 requests', { timeout: 15000 });
  expect(await output(page)).toBe(fullTable);
});

test('csv format emits a header and one escapable row per request', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-format', 'csv');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    [
      'index,method,url,status,status_text,mime_type,size_bytes,time_ms,started',
      '1,GET,https://example.com/,200,OK,text/html,2048,102.5,2024-01-01T00:00:00.000Z',
      '2,POST,https://example.com/api/search,200,OK,application/json,10240,812,2024-01-01T00:00:01.000Z',
      '3,GET,https://cdn.example.com/logo.png,404,Not Found,image/png,512,54,2024-01-01T00:00:02.000Z',
    ].join('\n'),
  );
});

test('urls format + slowest sort orders by total time', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-format', 'urls');
  await page.selectOption('#in-sort', 'slowest');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    [
      'https://example.com/api/search',
      'https://example.com/',
      'https://cdn.example.com/logo.png',
    ].join('\n'),
  );
});

test('json format + largest sort keeps capture indices and typed fields', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-format', 'json');
  await page.selectOption('#in-sort', 'largest');
  await expect(page.locator('#tool-output')).toContainText('"index"', { timeout: 15000 });
  const rows = JSON.parse(await output(page));
  expect(rows.map((r: { index: number }) => r.index)).toEqual([2, 1, 3]);
  expect(rows[0]).toEqual({
    index: 2,
    method: 'POST',
    url: 'https://example.com/api/search',
    status: 200,
    status_text: 'OK',
    mime_type: 'application/json',
    size_bytes: 10240,
    time_ms: 812.0,
    started: '2024-01-01T00:00:01.000Z',
  });
});

test('status class filters match advertised semantics (2xx/3xx/4xx/5xx/errors)', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  const firstLine = async () => (await output(page)).split('\n')[0];
  await page.selectOption('#in-status', '2xx');
  await expect.poll(firstLine, { timeout: 15000 }).toBe('2 of 3 requests · 12.0 KB transferred');
  await page.selectOption('#in-status', '3xx');
  await expect.poll(firstLine, { timeout: 15000 }).toBe('0 of 3 requests · 0 B transferred');
  await page.selectOption('#in-status', '4xx');
  await expect.poll(firstLine, { timeout: 15000 }).toBe('1 of 3 requests · 512 B transferred');
  await page.selectOption('#in-status', '5xx');
  await expect.poll(firstLine, { timeout: 15000 }).toBe('0 of 3 requests · 0 B transferred');
  await page.selectOption('#in-status', 'errors');
  await expect.poll(firstLine, { timeout: 15000 }).toBe('1 of 3 requests · 512 B transferred');
  await expect(page.locator('#tool-output')).toContainText('logo.png');
});

test('method and URL-substring filters compose case-insensitively', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', har);
  await page.fill('#in-method', 'post');
  await expect(page.locator('#tool-output')).toContainText('1 of 3 requests', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('/api/search');
  await page.fill('#in-method', '');
  await page.fill('#in-url_contains', 'CDN.example');
  await expect.poll(async () => (await output(page)).split('\n')[0], { timeout: 15000 }).toBe(
    '1 of 3 requests · 512 B transferred',
  );
  await expect(page.locator('#tool-output')).toContainText('logo.png');
});

test('non-HAR input errors with a clear message', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.fill('#in-har', '{"notlog":1}');
  await expect(page.locator('#tool-output')).toContainText(
    'not a HAR file: missing top-level "log" object',
    { timeout: 15000 },
  );
});

test('example chip prefills and runs the errors-only preset', async ({ page }) => {
  await page.goto('/tools/har-request-extract/');
  await page.click('button.tool-example-chip[data-example="1"]');
  await expect(page.locator('#in-status')).toHaveValue('errors', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('1 of 3 requests', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('logo.png');
});

test('deep link prefills every param and runs', async ({ page }) => {
  const params = new URLSearchParams({
    har,
    format: 'urls',
    status: '2xx',
    sort: 'slowest',
    url_contains: 'example.com',
  });
  await page.goto(`/tools/har-request-extract/?${params.toString()}`);
  await expect(page.locator('#in-har')).toHaveValue(har, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('urls');
  await expect(page.locator('#in-status')).toHaveValue('2xx');
  await expect(page.locator('#in-sort')).toHaveValue('slowest');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    ['https://example.com/api/search', 'https://example.com/'].join('\n'),
  );
});
