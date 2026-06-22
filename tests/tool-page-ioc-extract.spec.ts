import { test, expect } from './fixtures';

test('ioc-extract page extracts mixed IOCs from text', async ({ page }) => {
  await page.goto('/tools/ioc-extract/');

  // Defanged + real indicators of several types. types defaults to 'all',
  // defang checkbox defaults unchecked → real (refanged) output.
  await page.fill(
    '#in-text',
    'C2 at hxxp[://]evil[.]example[.]com from 203.0.113.5, phish bad[at]attacker[dot]net, drop d41d8cd98f00b204e9800998ecf8427e',
  );

  const out = page.locator('#tool-output');
  await expect(out).toContainText('203.0.113.5', { timeout: 15000 });
  await expect(out).toContainText('http://evil.example.com');
  await expect(out).toContainText('bad@attacker.net');
  await expect(out).toContainText('d41d8cd98f00b204e9800998ecf8427e');
});

test('ioc-extract page filters by type and re-defangs output', async ({ page }) => {
  await page.goto('/tools/ioc-extract/');

  await page.fill('#in-text', 'visit http://evil.com and good.org and bad@x.com');
  await page.fill('#in-types', 'url,domain');
  await page.check('#in-defang');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('hxxp[://]evil[.]com', { timeout: 15000 });
  await expect(out).toContainText('good[.]org');
  // email excluded by the type filter; URL host not double-listed as a domain.
  await expect(out).not.toContainText('bad@x.com');
});
