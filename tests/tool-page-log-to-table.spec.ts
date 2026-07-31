import { test, expect } from './fixtures';

async function setLogs(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-logs').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('log-to-table parses custom logs into CSV with exact output', async ({ page }) => {
  await page.goto('/tools/log-to-table/');
  await setLogs(page, 'ERROR 42 failed to connect\nINFO 7 retry scheduled');
  await page.locator('#in-pattern').fill('^(?P<level>\\w+) (?P<code>\\d+) (?P<message>.*)$');
  await page.selectOption('#in-output', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('level,code,message', { timeout: 15_000 });
  expect(await out.textContent()).toBe('level,code,message\nERROR,42,failed to connect\nINFO,7,retry scheduled');
});

test('log-to-table deep-link parses Apache combined preset', async ({ page }) => {
  const logs = '127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "-" "Mozilla/5.0"';
  const qs = new URLSearchParams({ logs, preset: 'combined', output: 'csv' });
  await page.goto(`/tools/log-to-table/?${qs.toString()}`);

  await expect(page.locator('#in-logs')).toHaveValue(logs);
  await expect(page.locator('#in-preset')).toHaveValue('combined');
  await expect(page.locator('#in-output')).toHaveValue('csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('host,user,time,method,path,protocol,status,size,referer,agent', { timeout: 15_000 });
  expect(await out.textContent()).toBe('host,user,time,method,path,protocol,status,size,referer,agent\n127.0.0.1,-,10/Oct/2000:13:55:36 -0700,GET,/index.html,HTTP/1.0,200,2326,-,Mozilla/5.0');
});

test('log-to-table keep unmatched and no header', async ({ page }) => {
  await page.goto('/tools/log-to-table/');
  await setLogs(page, 'ok 1\ngarbage\nok 2');
  await page.locator('#in-pattern').fill('^(?P<word>\\w+) (?P<count>\\d+)$');
  await page.selectOption('#in-output', 'csv');
  await page.selectOption('#in-on_nomatch', 'keep');
  await page.locator('#in-header').uncheck();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('ok,1,', { timeout: 15_000 });
  expect(await out.textContent()).toBe('ok,1,\n,,garbage\nok,2,');
});

test('log-to-table limit boundary outputs one row', async ({ page }) => {
  await page.goto('/tools/log-to-table/');
  await setLogs(page, 'a 1\nb 2');
  await page.locator('#in-pattern').fill('^(?P<name>\\w+) (?P<n>\\d+)$');
  await page.selectOption('#in-output', 'csv');
  await page.locator('#in-limit').fill('1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,n\na,1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('name,n\na,1');
});
