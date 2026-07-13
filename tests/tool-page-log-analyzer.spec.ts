import { test, expect } from './fixtures';

// /tools/log-analyzer/ summarizes raw logs in-browser (pure wasm). Tests assert
// real output across the page surface, including enum controls and deep links.

const JSON_LOGS = [
  '{"ts":"2024-01-01T00:00:00Z","level":"info","msg":"server started"}',
  '{"ts":"2024-01-01T00:20:00Z","level":"warn","msg":"high latency 900ms"}',
  '{"ts":"2024-01-01T00:40:00Z","level":"error","msg":"db timeout attempt 3"}',
  '{"ts":"2024-01-01T00:41:00Z","level":"error","msg":"db timeout attempt 5"}',
].join('\n');

const COMBINED = [
  '127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "-" "Mozilla/5.0"',
  '10.0.0.5 - - [10/Oct/2000:13:55:37 -0700] "GET /missing HTTP/1.1" 404 512 "-" "curl/7.0"',
  '10.0.0.9 - - [10/Oct/2000:13:55:38 -0700] "POST /api HTTP/1.1" 500 12 "-" "Go-http-client/1.1"',
].join('\n');

test('log-analyzer summarizes JSON logs with grouped top errors', async ({ page }) => {
  await page.goto('/tools/log-analyzer/');
  await page.fill('#in-logs', JSON_LOGS);
  await page.fill('#in-top', '2');
  await page.selectOption('#in-bucket', 'minute');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('json · 4 entries · 2024-01-01 00:00:00 → 2024-01-01 00:41:00', { timeout: 15000 });
  await expect(out).toContainText('| error | 2 | 50.0% |');
  await expect(out).toContainText('| 2 | error | db timeout attempt # |');
  await expect(out).toContainText('## Volume timeline (per minute)');
});

test('log-analyzer JSON output is a structured object', async ({ page }) => {
  await page.goto('/tools/log-analyzer/');
  await page.fill('#in-logs', JSON_LOGS);
  await page.selectOption('#in-output', 'json');
  const text = await page.locator('#tool-output').textContent({ timeout: 15000 });
  const parsed = JSON.parse(text!.trim());
  expect(parsed.format).toBe('json');
  expect(parsed.total).toBe(4);
  expect(parsed.levels.error).toBe(2);
  expect(parsed.top_errors[0].message).toBe('db timeout attempt #');
});

test('log-analyzer handles combined access logs and forced format', async ({ page }) => {
  await page.goto('/tools/log-analyzer/');
  await page.fill('#in-logs', COMBINED);
  await page.selectOption('#in-format', 'combined');
  await page.selectOption('#in-bucket', 'hour');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('combined · 3 entries', { timeout: 15000 });
  await expect(out).toContainText('| error | 1 | 33.3% |');
  await expect(out).toContainText('| warn | 1 | 33.3% |');
  await expect(out).toContainText('POST /api HTTP/#.# #');
});

test('log-analyzer deep-link pre-fills and computes', async ({ page }) => {
  const logs = encodeURIComponent(JSON_LOGS);
  await page.goto(`/tools/log-analyzer/?logs=${logs}&format=json&output=json&top=1&bucket=minute`);
  const text = await page.locator('#tool-output').textContent({ timeout: 15000 });
  const parsed = JSON.parse(text!.trim());
  expect(parsed.format).toBe('json');
  expect(parsed.top_errors).toHaveLength(1);
  expect(parsed.timeline.bucket).toBe('minute');
  expect(parsed.timeline.points[0].time).toBe('2024-01-01 00:00');
});
