import { test, expect } from './fixtures';

const CLEAN_HEADERS = `From: Example Alerts <alerts@example.com>
Return-Path: <bounce@example.com>
Authentication-Results: mx.example; spf=pass smtp.mailfrom=example.com; dkim=pass header.d=example.com; dmarc=pass header.from=example.com
Received: from mail.example.com by mx.example; Tue, 1 Jan 2026 00:00:00 +0000
Received: from app.example.com by mail.example.com; Tue, 1 Jan 2026 00:00:01 +0000
Message-ID: <abc@example.com>`;

const SPOOFED_HEADERS = `From: "alerts@paypal.com" <notice@evil.example>
Return-Path: <bounce@mailer.bad>
Reply-To: help@bad.example
Authentication-Results: mx.example; spf=fail smtp.mailfrom=mailer.bad; dkim=none; dmarc=fail header.from=evil.example
Received: from [10.0.0.4] by mx.example; Tue, 1 Jan 2026 00:00:00 +0000
Message-ID: <abc@mailer.bad>`;

test('phishing-header-inspector reports minimal risk for aligned authenticated headers', async ({ page }) => {
  await page.goto('/tools/phishing-header-inspector/');
  await page.fill('#in-headers', CLEAN_HEADERS);
  await page.selectOption('#in-report_mode', 'detailed');
  await expect(page.locator('#tool-output')).toContainText('Risk: MINIMAL (0/100)', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Authentication: SPF pass; DKIM pass; DMARC pass');
  await expect(page.locator('#tool-output')).toContainText('Received hops: 2');
});

test('phishing-header-inspector flags spoofing indicators in summary mode', async ({ page }) => {
  await page.goto('/tools/phishing-header-inspector/');
  await page.fill('#in-headers', SPOOFED_HEADERS);
  await page.selectOption('#in-report_mode', 'summary');
  await expect(page.locator('#tool-output')).toContainText('Risk: CRITICAL', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('From domain evil.example differs from Return-Path domain mailer.bad.');
  await expect(page.locator('#tool-output')).toContainText('Authentication: SPF fail; DKIM none; DMARC fail');
});

test('phishing-header-inspector deep-link honors enum and checkbox params', async ({ page }) => {
  const qs = new URLSearchParams({
    headers: 'From: Alerts <alerts@example.com>\nAuthentication-Results: mx.example; spf=pass; dkim=pass; dmarc=pass',
    report_mode: 'summary',
    check_received: 'false',
  });
  await page.goto('/tools/phishing-header-inspector/?' + qs.toString());
  await expect(page.locator('#tool-output')).toContainText('Risk: LOW (10/100)', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Received hops: 0');
  await expect(page.locator('#tool-output')).not.toContainText('No Received headers were found');
});
