import { test, expect } from './fixtures';

const msg = 'what do ya want for nothing?';
const key = 'Jefe';
const tag = '5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843';

test('hmac-verify page reports MATCH and MISMATCH with exact computed tag', async ({ page }) => {
  await page.goto('/tools/hmac-verify/');
  await page.fill('#in-message', msg);
  await page.fill('#in-key', key);
  await page.fill('#in-expected', tag);
  await page.selectOption('#in-algorithm', 'sha256');
  await page.selectOption('#in-message_encoding', 'text');
  await page.selectOption('#in-key_encoding', 'text');
  await page.selectOption('#in-expected_encoding', 'hex');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('status:    MATCH', { timeout: 15000 });
  await expect(out).toContainText(`computed:  ${tag}`);

  await page.fill('#in-expected', tag.slice(0, -1) + '4');
  await expect(out).toContainText('status:    MISMATCH', { timeout: 15000 });
});

test('hmac-verify supports deep-link params and base64 expected tags', async ({ page }) => {
  const qs = new URLSearchParams({
    message: msg,
    key,
    expected: 'W9zBRr9gdU5qBCQmCJV1x1oAPwidJzmDnexYuWTsOEM=',
    algorithm: 'sha256',
    message_encoding: 'text',
    key_encoding: 'text',
    expected_encoding: 'base64',
  });

  await page.goto('/tools/hmac-verify/?' + qs.toString());

  await expect(page.locator('#in-expected_encoding')).toHaveValue('base64', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('status:    MATCH', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText(`expected:  ${tag}`);
});
