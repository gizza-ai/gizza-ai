import { test, expect } from './fixtures';

const LOG = [
  '1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"',
  '1.1.1.1 - - [06/Aug/2026:10:05:00 +0000] "GET /a HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"',
  '2.2.2.2 - - [06/Aug/2026:11:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Safari/604.1"',
  '1.1.1.1 - - [07/Aug/2026:09:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"',
].join('\n');

async function runWasm(
  page: import('@playwright/test').Page,
  input: string,
  format = 'auto',
  identity = 'ip_ua',
  period = 'day',
  salt = '',
  excludeBots = 'true',
  hashLength = '12',
  output = 'report',
) {
  return await page.evaluate(async ({ input, format, identity, period, salt, excludeBots, hashLength, output }) => {
    const mod = await import('/tools/cookieless-visitor-counter/gizza_ai_cookieless_visitor_counter_web.js');
    await mod.default('/tools/cookieless-visitor-counter/gizza_ai_cookieless_visitor_counter_web_bg.wasm');
    return mod.run(input, format, identity, period, salt, excludeBots, hashLength, output);
  }, { input, format, identity, period, salt, excludeBots, hashLength, output });
}

test('cookieless-visitor-counter wasm returns the daily report exactly', async ({ page }) => {
  await page.goto('/tools/cookieless-visitor-counter/');
  await page.waitForSelector('#in-input');

  const out = await runWasm(page, LOG, 'combined', 'ip_ua', 'day', '', 'true', '12', 'report');
  expect(out).toBe([
    'Cookieless visitor count',
    '========================',
    'Method:    daily-salted-hash (SHA-256), no cookies, no PII stored',
    'Identity:  IP + user-agent',
    'Bucket:    daily',
    'Format:    Combined log',
    '',
    'Date        Visitors  Pageviews  Views/visitor',
    '----------  --------  ---------  -------------',
    '2026-08-06         2          3           1.50',
    '2026-08-07         1          1           1.00',
    '',
    'Total pageviews:        4',
    'Sum of daily uniques:  3',
    'Distinct visitors:      2 (across the whole log)',
    'Requests parsed:        4',
    'Bot hits excluded:      0',
    '',
    'Note: per-daily IDs are un-linkable across periods by design, so the',
    'sum above double-counts anyone who returned. 2 people are distinct.',
  ].join('\n'));
});

test('cookieless-visitor-counter wasm covers advertised formats, enums and values', async ({ page }) => {
  await page.goto('/tools/cookieless-visitor-counter/');
  await page.waitForSelector('#in-input');

  const hourly = await runWasm(page, LOG, 'combined', 'ip_ua', 'hour', 'site-salt', 'true', '16', 'json');
  expect(hourly).toContain('"bucket": "hourly"');
  expect(hourly).toContain('"period": "2026-08-06 10:00"');

  const ipOnly = await runWasm(page,
    '9.9.9.9 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Chrome"\n' +
    '9.9.9.9 - - [06/Aug/2026:10:01:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Firefox"',
    'combined', 'ip', 'day', '', 'true', '12', 'csv');
  expect(ipOnly).toContain('2026-08-06,1,2,2.00');

  const network = await runWasm(page,
    'timestamp,ip,user_agent\n2026-08-06 10:00:00,203.0.113.4,UA/1\n2026-08-06 10:01:00,203.0.113.9,UA/1',
    'csv', 'network_ua', 'month', '', 'true', '12', 'table');
  expect(network).toContain('| 2026-08 | 1 | 2 | 2.00 |');

  const ids = await runWasm(page, '{"ip":"1.1.1.1","user_agent":"Mozilla","time":"2026-08-06T10:00:00Z"}', 'json', 'ip_ua', 'total', '', 'false', '6', 'ids');
  expect(ids).toMatch(/\| 1 \| all \| [0-9a-f]{6} \|/);

  await expect(runWasm(page, LOG, 'bad')).rejects.toThrow(/unknown format 'bad'/);
});

test('cookieless-visitor-counter page renders output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/cookieless-visitor-counter/');
  await page.fill('#in-input', '1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 1 "-" "Googlebot/2.1"');
  await page.selectOption('#in-format', 'combined');
  await page.uncheck('#in-exclude_bots');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toHaveText('date,visitors,pageviews,views_per_visitor\n2026-08-06,1,1,1.00', { timeout: 15_000 });
});

test('cookieless-visitor-counter deep-link prefills fields and emits JSON', async ({ page }) => {
  const qs = new URLSearchParams({
    input: LOG,
    format: 'combined',
    identity: 'ip_ua',
    period: 'day',
    salt: 'site-salt',
    exclude_bots: 'true',
    hash_length: '16',
    output: 'json',
  });
  await page.goto(`/tools/cookieless-visitor-counter/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(LOG, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('combined');
  await expect(page.locator('#in-identity')).toHaveValue('ip_ua');
  await expect(page.locator('#in-period')).toHaveValue('day');
  await expect(page.locator('#in-salt')).toHaveValue('site-salt');
  await expect(page.locator('#in-exclude_bots')).toBeChecked();
  await expect(page.locator('#in-hash_length')).toHaveValue('16');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"distinct_visitors": 2', { timeout: 15_000 });
});
