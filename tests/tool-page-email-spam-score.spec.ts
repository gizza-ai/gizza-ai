import { test, expect } from './fixtures';

const cleanRaw = `From: Ada Lovelace <ada@example.com>
To: team@example.com
Return-Path: <ada@example.com>
Date: Tue, 1 Jan 2026 09:00:00 +0000
Message-ID: <a1@example.com>
Subject: Notes from planning
Received: from mail.example.com by mx.example.com
Authentication-Results: mx.example.com; spf=pass; dkim=pass; dmarc=pass

Hi team, here are the notes from planning. https://docs.example.com/plan
`;

const cleanSummary = `Spam score: 0/100 (LOW)
Verdict: reads as legitimate to rules-based spam filters
Input parsed as: raw email with a plain-text body
Top signals: none`;

const spamText = 'ACT NOW! YOUR ACCOUNT SUSPENDED. CLICK HERE TO VERIFY YOUR ACCOUNT AND CLAIM YOUR FREE GIFT OF $50,000 CASH!!! http://bit.ly/x1 http://192.0.2.9/login';

test('email-spam-score page emits exact clean summary output', async ({ page }) => {
  await page.goto('/tools/email-spam-score/');
  await page.fill('#in-email', cleanRaw);
  await page.selectOption('#in-report', 'summary');

  await expect(page.locator('#tool-output')).toHaveText(cleanSummary, { timeout: 15_000 });
});

test('email-spam-score honours deep-link JSON body-only params', async ({ page }) => {
  const qs = new URLSearchParams({
    email: spamText,
    subject: 'URGENT!! VERIFY YOUR ACCOUNT NOW!!',
    format: 'text',
    report: 'json',
    check_headers: 'false',
  });

  await page.goto(`/tools/email-spam-score/?${qs.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('text', { timeout: 15_000 });
  await expect(page.locator('#in-report')).toHaveValue('json');
  await expect(page.locator('#in-check_headers')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"score": 95', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"band": "CRITICAL"');
  await expect(page.locator('#tool-output')).toContainText('"id": "CAPS_RATIO"');
  await expect(page.locator('#tool-output')).toContainText('"links": 2');
});

test('email-spam-score covers html format and header checkbox toggle', async ({ page }) => {
  await page.goto('/tools/email-spam-score/');
  await page.fill(
    '#in-email',
    '<html><body><h1>Limited offer</h1><img src="track.gif" width="1" height="1"><div style="display:none">hidden keywords</div><a href="https://evil.example/login">https://secure.example.com/login</a></body></html>',
  );
  await page.selectOption('#in-format', 'html');
  await page.selectOption('#in-report', 'detailed');
  await page.uncheck('#in-check_headers');

  await expect(page.locator('#tool-output')).toContainText('Input parsed as: HTML body (no headers)', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('IMAGE_HEAVY');
  await expect(page.locator('#tool-output')).toContainText('TRACKING_PIXEL');
  await expect(page.locator('#tool-output')).toContainText('HIDDEN_TEXT');
  await expect(page.locator('#tool-output')).toContainText('URL_MISMATCH');
  await expect(page.locator('#tool-output')).not.toContainText('AUTH_MISSING');
});
