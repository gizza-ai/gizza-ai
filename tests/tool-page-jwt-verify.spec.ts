import { test, expect } from './fixtures';

// /tools/jwt-verify/ verifies a JWT signature + claims in-browser (pure wasm).
// token + key are multiline <textarea>; algorithm/issuer/audience/leeway are <input>.
// The output is a pretty-printed JSON report with a `valid` flag and a `checks[]` array.

function b64url(s: string): string {
  return Buffer.from(s, 'utf-8')
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

// A pre-computed HS256 token for secret "topsecret" with payload {"sub":"123"}.
// header {"alg":"HS256","typ":"JWT"} . payload . HMAC-SHA256(signing_input, secret)
// Computed offline; the wasm recomputes the HMAC and must agree.
const SECRET = 'topsecret';
// Build the signing input deterministically; the signature below is the HMAC of it.
const HEADER_B64 = b64url('{"alg":"HS256","typ":"JWT"}');
const PAYLOAD_B64 = b64url('{"sub":"123","exp":4102444800}');

import { createHmac } from 'crypto';
function sign(secret: string): string {
  const si = `${HEADER_B64}.${PAYLOAD_B64}`;
  const sig = createHmac('sha256', secret)
    .update(si)
    .digest('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
  return `${si}.${sig}`;
}

test('jwt-verify page accepts a valid HS256 token', async ({ page }) => {
  await page.goto('/tools/jwt-verify/');
  await page.fill('#in-token', sign(SECRET));
  await page.fill('#in-key', SECRET);
  await page.selectOption('#in-algorithm', 'HS256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const report = JSON.parse((await out.textContent())!.trim());
  expect(report.valid).toBe(true);
  expect(report.algorithm).toBe('HS256');
  expect(report.payload.sub).toBe('123');
  const sigCheck = report.checks.find((c: any) => c.check === 'signature');
  expect(sigCheck.ok).toBe(true);
});

test('jwt-verify page rejects a wrong secret', async ({ page }) => {
  await page.goto('/tools/jwt-verify/');
  await page.fill('#in-token', sign(SECRET));
  await page.fill('#in-key', 'not-the-secret');
  await page.selectOption('#in-algorithm', 'HS256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const report = JSON.parse((await out.textContent())!.trim());
  expect(report.valid).toBe(false);
  expect(report.error).toContain('signature');
});

test('jwt-verify page enforces a required algorithm (alg-confusion)', async ({ page }) => {
  await page.goto('/tools/jwt-verify/');
  await page.fill('#in-token', sign(SECRET));
  await page.fill('#in-key', SECRET);
  await page.selectOption('#in-algorithm', 'RS256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const report = JSON.parse((await out.textContent())!.trim());
  expect(report.valid).toBe(false);
  expect(report.error).toContain('RS256');
});

test('jwt-verify page errors clearly on a malformed token', async ({ page }) => {
  await page.goto('/tools/jwt-verify/');
  await page.fill('#in-token', 'not-a-jwt');
  await page.fill('#in-key', SECRET);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3', { timeout: 15000 }); // "expected 3 dot-separated parts"
});
