import { test, expect } from './fixtures';

const AUTH = [
  'May  3 18:20:45 web1 sshd[2001]: Failed password for root from 203.0.113.5 port 44001 ssh2',
  'May  3 18:20:47 web1 sshd[2002]: Failed password for invalid user admin from 203.0.113.5 port 44002 ssh2',
  'May  3 18:21:10 web1 sshd[2010]: Accepted publickey for bob from 192.168.1.10 port 51000 ssh2',
  'May  3 18:22:00 web1 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt-get update',
  'May  3 18:25:01 web1 CRON[3001]: (root) CMD (/usr/local/bin/backup.sh)',
  'May  3 18:26:00 web1 su[3100]: pam_unix(su:session): session opened for user root by alice(uid=1000)',
].join('\n');

async function setLogs(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-logs').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('syslog-triage renders an intrusion-review summary with exact output', async ({ page }) => {
  await page.goto('/tools/syslog-triage/');
  await setLogs(page, AUTH);

  const expected = [
    'Syslog triage · 6 events · 2 failed',
    '',
    'Categories: sudo 1 · ssh 3 · cron 1 · session 1',
    '',
    'Failed logins by source IP:',
    '  203.0.113.5 ×2 (users: root, admin)',
    '',
    'Sudo activity:',
    '  alice ran (as root) /usr/bin/apt-get update',
    '',
    'Cron:',
    '  (root) ran /usr/local/bin/backup.sh',
  ].join('\n');

  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15_000 });
});

test('syslog-triage deep-link filters failed ssh rows as a markdown table', async ({ page }) => {
  const qs = new URLSearchParams({
    logs: AUTH,
    category: 'ssh',
    only: 'failed',
    output: 'table',
    limit: '2',
  });
  await page.goto(`/tools/syslog-triage/?${qs.toString()}`);

  await expect(page.locator('#in-logs')).toHaveValue(AUTH);
  await expect(page.locator('#in-category')).toHaveValue('ssh');
  await expect(page.locator('#in-only')).toHaveValue('failed');
  await expect(page.locator('#in-output')).toHaveValue('table');
  await expect(page.locator('#in-limit')).toHaveValue('2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Syslog triage · 2 events · 2 failed', { timeout: 15_000 });
  await expect(out).toContainText('| ssh | failure | root | 203.0.113.5 |');
  await expect(out).toContainText('| ssh | failure | admin | 203.0.113.5 |');
  await expect(out).not.toContainText('bob');
});
