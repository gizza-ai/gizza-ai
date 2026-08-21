import { test, expect } from './fixtures';

const sample = [
  '203.0.113.5 - - [11/Mar/2024:09:14:02 +0000] "GET /products.php?id=1%27+UNION+SELECT+null,version()--+- HTTP/1.1" 200 512 "-" "sqlmap/1.7"',
  '203.0.113.5 - - [11/Mar/2024:09:14:05 +0000] "GET /admin/../../etc/passwd HTTP/1.1" 404 153 "-" "sqlmap/1.7"',
  '198.51.100.22 - - [11/Mar/2024:09:14:20 +0000] "GET /search?q=%3Cscript%3Ealert(1)%3C/script%3E HTTP/1.1" 200 980 "-" "Mozilla/5.0"',
].join('\n');

test('weblog-attack-analyzer flags SQLi, traversal, XSS, and scanner UAs', async ({ page }) => {
  await page.goto('/tools/weblog-attack-analyzer/');
  await page.fill('#in-logs', sample);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Weblog attack analysis · combined · 3 requests · 3 flagged · 2 source IPs', { timeout: 15000 });
  await expect(out).toContainText('sqli');
  await expect(out).toContainText('traversal');
  await expect(out).toContainText('xss');
  await expect(out).toContainText('scanner UA: sqlmap/1.7');
  await expect(out).toContainText('after percent-decoding');
});

test('weblog-attack-analyzer supports query params for CSV critical-only output', async ({ page }) => {
  const params = new URLSearchParams({
    logs: sample,
    min_severity: 'critical',
    output: 'csv',
    decode: 'true',
  });
  await page.goto(`/tools/weblog-attack-analyzer/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('line,severity,category,ip,time,method,target,status,user_agent,matched', { timeout: 15000 });
  await expect(out).toContainText('1,critical,sqli,203.0.113.5');
  await expect(out).not.toContainText('traversal');
});

test('weblog-attack-analyzer exposes blocklist mode', async ({ page }) => {
  await page.goto('/tools/weblog-attack-analyzer/');
  await page.fill('#in-logs', sample);
  await page.selectOption('#in-output', 'blocklist');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('203.0.113.5', { timeout: 15000 });
  await expect(out).toContainText('198.51.100.22');
});
