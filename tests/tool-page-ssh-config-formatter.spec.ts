import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const messyConfig = 'Host  web\nhostname=10.0.0.5\n   User deploy\n  port 2222';

test('ssh-config-formatter formats messy config exactly', async ({ page }) => {
  await page.goto('/tools/ssh-config-formatter/');
  await setValue(page, '#in-text', messyConfig);
  await page.selectOption('#in-output', 'formatted');
  await setValue(page, '#in-indent', '2');

  await expect(page.locator('#tool-output')).toHaveText(
    'Host web\n  HostName 10.0.0.5\n  User deploy\n  Port 2222\n\n# ssh-config-formatter: no issues found',
    { timeout: 15_000 },
  );
});

test('ssh-config-formatter deep-links report output and lint findings', async ({ page }) => {
  const text = 'Host *\n  User root\n\nHost web\n  HostName 10.0.0.5\n  Port 70000\n  PermitRootLogin no';
  const qs = new URLSearchParams({
    text,
    output: 'report',
    indent: '4',
    keyword_case: 'canonical',
    align_values: 'false',
    sort_keywords: 'false',
    dedupe: 'false',
    include_notes: 'true',
    min_severity: 'warning',
  });
  await page.goto(`/tools/ssh-config-formatter/?${qs.toString()}`);

  await expect(page.locator('#in-text')).toHaveValue(text, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-indent')).toHaveValue('4');
  await expect(page.locator('#in-min_severity')).toHaveValue('warning');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 Host, 0 Match, 4 directive(s)', { timeout: 15_000 });
  await expect(out).toContainText('`Host *` matches every host but 1 later block(s) follow it');
  await expect(out).toContainText('`Port 70000` is out of range (1–65535)');
  await expect(out).toContainText('`PermitRootLogin` is an sshd_config (server) keyword');
});

test('ssh-config-formatter covers enum choices and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/ssh-config-formatter/');
  await setValue(page, '#in-text', 'Host web prod\n  user deploy\n  hostname example.com\n  user ignored\nHost db\n  HostName db.internal');
  await page.selectOption('#in-output', 'hosts');
  await page.selectOption('#in-keyword_case', 'lower');
  await page.selectOption('#in-min_severity', 'error');
  await setValue(page, '#in-indent', '0');
  await page.locator('#in-align_values').check();
  await page.locator('#in-sort_keywords').check();
  await page.locator('#in-dedupe').check();
  await page.locator('#in-include_notes').uncheck();

  await expect(page.locator('#tool-output')).toHaveText('web\nprod\ndb', { timeout: 15_000 });

  await page.selectOption('#in-output', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"hosts"', { timeout: 15_000 });
  await expect(out).toContainText('"web"');
  await expect(out).toContainText('"stats"');
});
