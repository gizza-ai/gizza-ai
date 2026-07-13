import { test, expect } from './fixtures';

test('url-safety-inspect flags the disguised IP + @ trick as CRITICAL', async ({ page }) => {
  await page.goto('/tools/url-safety-inspect/');
  await page.fill('#in-url', 'http://paypal.com@192.168.0.1/login');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Phishing risk: CRITICAL', { timeout: 15000 });
  await expect(out).toContainText("the browser connects to '192.168.0.1'");
  await expect(out).toContainText('Host is an IP literal');
});

test('url-safety-inspect rates a clean URL as MINIMAL with no findings', async ({ page }) => {
  await page.goto('/tools/url-safety-inspect/');
  await page.fill('#in-url', 'https://www.example.com/pricing');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Phishing risk: MINIMAL (score 0/100)', { timeout: 15000 });
  await expect(out).toContainText('no structural red flags');
});

test('url-safety-inspect deep-links a punycode + free-TLD URL via ?url=', async ({ page }) => {
  await page.goto('/tools/url-safety-inspect/?url=https%3A%2F%2Fxn--pple-43d.tk%2Fid%2Fverify');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('punycode label', { timeout: 15000 });
  await expect(out).toContainText("TLD '.tk'");
});
