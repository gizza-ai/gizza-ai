import { test, expect } from './fixtures';

// /tools/log-parser/ auto-detects and parses raw logs into a table / JSON / CSV
// (pure wasm, in-browser). format/output/level are <select>; regex is a
// checkbox; logs/filter/limit are fields. Every test asserts real parsed output.

const COMBINED = [
  '127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "-" "Mozilla/5.0"',
  '10.0.0.5 - - [10/Oct/2000:13:55:37 -0700] "GET /missing HTTP/1.1" 404 512 "-" "curl/7.0"',
  '10.0.0.9 - - [10/Oct/2000:13:55:38 -0700] "POST /api HTTP/1.1" 500 12 "-" "Go-http-client/1.1"',
].join('\n');

const JSON_LOGS = [
  '{"ts":"2024-01-01T00:00:00Z","level":"info","msg":"started"}',
  '{"ts":"2024-01-01T00:00:09Z","level":"error","msg":"boom"}',
].join('\n');

test('log-parser auto-detects a combined access log and maps status→severity', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', COMBINED);
  const out = page.locator('#tool-output');
  // caption: 1 error (500) + 1 warn (404) over 3 entries
  await expect(out).toContainText('combined · 3 entries · 1 error · 1 warn', { timeout: 15000 });
  await expect(out).toContainText('| ip | ident | user | time | request | status | size | referer | user_agent |');
  await expect(out).toContainText('| 10.0.0.9 | - | - |');
  await expect(out).toContainText('POST /api HTTP/1.1');
});

test('log-parser auto-detects JSON lines', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', JSON_LOGS);
  await expect(page.locator('#tool-output')).toContainText('json · 2 entries', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('| ts | level | msg |');
});

test('log-parser auto-detects logfmt', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', 'level=info msg="server up" port=8080\nlevel=warn msg="slow query" ms=1200');
  await expect(page.locator('#tool-output')).toContainText('logfmt · 2 entries', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('slow query');
});

test('log-parser parses syslog with PRI severity', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', '<34>Oct 11 22:14:15 mymachine su: authentication failure');
  await page.selectOption('#in-format', 'syslog');
  await expect(page.locator('#tool-output')).toContainText('syslog · 1 entries · 1 error', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('authentication failure');
});

test('log-parser JSON output is a valid array', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', JSON_LOGS);
  await page.selectOption('#in-output', 'json');
  const text = await page.locator('#tool-output').textContent();
  const parsed = JSON.parse(text!.trim());
  expect(Array.isArray(parsed)).toBe(true);
  expect(parsed.length).toBe(2);
  expect(parsed[1].msg).toBe('boom');
});

test('log-parser CSV output has a header row', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', COMBINED);
  await page.selectOption('#in-format', 'combined');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toContainText(
    'ip,ident,user,time,request,status,size,referer,user_agent',
    { timeout: 15000 },
  );
});

test('log-parser minimum-severity filter keeps only errors', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', COMBINED);
  await page.selectOption('#in-level', 'error');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('· 1 shown', { timeout: 15000 });
  await expect(out).toContainText('POST /api HTTP/1.1');
  await expect(out).not.toContainText('/index.html');
});

test('log-parser regex filter (non-default checkbox on)', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', COMBINED);
  await page.fill('#in-filter', '\\b50\\d\\b');
  await page.check('#in-regex');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('· 1 shown', { timeout: 15000 });
  await expect(out).toContainText('POST /api HTTP/1.1');
  await expect(out).not.toContainText('/missing');
});

test('log-parser row limit caps the table', async ({ page }) => {
  await page.goto('/tools/log-parser/');
  await page.fill('#in-logs', COMBINED);
  await page.fill('#in-limit', '1');
  await expect(page.locator('#tool-output')).toContainText('· 1 shown', { timeout: 15000 });
});

test('log-parser deep-link pre-fills and computes', async ({ page }) => {
  const logs = encodeURIComponent(JSON_LOGS);
  await page.goto(`/tools/log-parser/?logs=${logs}&format=json&output=json&level=error`);
  const text = await page.locator('#tool-output').textContent();
  const parsed = JSON.parse(text!.trim());
  expect(Array.isArray(parsed)).toBe(true);
  expect(parsed.length).toBe(1);
  expect(parsed[0].msg).toBe('boom');
});
