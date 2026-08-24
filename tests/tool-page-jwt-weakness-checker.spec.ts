import { test, expect } from './fixtures';
import { createHmac } from 'node:crypto';

function b64urlJson(value: unknown): string {
  return Buffer.from(JSON.stringify(value)).toString('base64url');
}

function hs256(payload: Record<string, unknown>, secret: string, header: Record<string, unknown> = { alg: 'HS256', typ: 'JWT' }): string {
  const head = b64urlJson(header);
  const body = b64urlJson(payload);
  const sig = createHmac('sha256', secret).update(`${head}.${body}`).digest('base64url');
  return `${head}.${body}.${sig}`;
}

const NOW = Math.floor(Date.now() / 1000);
const WEAK_TOKEN = hs256({ sub: '123', name: 'Ada', iat: NOW }, 'secret');
const CLEAN_TOKEN = hs256({ sub: '123', iss: 'auth', aud: 'api', iat: NOW, exp: NOW + 3600 }, 'correct horse battery staple 1234567890');

test('jwt-weakness-checker reports a cracked HMAC secret', async ({ page }) => {
  await page.goto('/tools/jwt-weakness-checker/');
  await page.fill('#in-token', WEAK_TOKEN);
  await page.fill('#in-max_exp_days', '30');
  await page.fill('#in-leeway', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"id": "weak_secret"', { timeout: 15000 });
  await expect(out).toContainText('"cracked_secret": "secret"');
  await expect(out).toContainText('"risk_level": "critical"');
});

test('jwt-weakness-checker deep-links params and reports clean token floor', async ({ page }) => {
  await page.goto(
    '/tools/jwt-weakness-checker/?' +
      new URLSearchParams({
        token: CLEAN_TOKEN,
        wordlist: 'not-this-one',
        max_exp_days: '1',
        leeway: '120',
      }).toString()
  );

  await expect(page.locator('#in-token')).toHaveValue(CLEAN_TOKEN, { timeout: 15000 });
  await expect(page.locator('#in-wordlist')).toHaveValue('not-this-one');
  await expect(page.locator('#in-max_exp_days')).toHaveValue('1');
  await expect(page.locator('#in-leeway')).toHaveValue('120');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"risk_level": "low"', { timeout: 15000 });
  await expect(out).toContainText('"findings": []');
  await expect(out).toContainText('"algorithm": "HS256"');
});
