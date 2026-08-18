import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const INDEX_PREFIX = '1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b830\n3-732b46797f86f75b9aec7d';
const SSS_BASE64 = 'sss:AQ8hBuB2RlYOWc6_YNs\nsss:Arvzmk1DKTB8CjbzVQI\nsss:A9e97t9QDBJSO5c-Rrw';
const TRAILING_11D = '7b2d29cba93975ea5108479560c3b6bb01\nc5b32e7f030860ac928276c4f6ae75d402\nc8ff72d8de1c7827b0fe5423bb06a61603\n93ed6857fd5383d902bd028a9fe6d3ee04';

test('shamir-secret-recover combines index-prefixed shares exactly', async ({ page }) => {
  await page.goto('/tools/shamir-secret-recover/');
  await setValue(page, '#in-shares', INDEX_PREFIX);
  await setValue(page, '#in-threshold', '3');
  await page.selectOption('#in-output', 'secret');

  await expect(page.locator('#tool-output')).toHaveText('hello world', { timeout: 15_000 });
});

test('shamir-secret-recover deep-links sss base64url shares and report output', async ({ page }) => {
  const qs = new URLSearchParams({
    shares: SSS_BASE64,
    share_format: 'leading-index',
    share_encoding: 'base64',
    field_poly: 'auto',
    threshold: '2',
    verify: 'true',
    secret_encoding: 'text',
    output: 'report',
  });
  await page.goto(`/tools/shamir-secret-recover/?${qs.toString()}`);

  await expect(page.locator('#in-shares')).toHaveValue(SSS_BASE64, { timeout: 15_000 });
  await expect(page.locator('#in-share_format')).toHaveValue('leading-index');
  await expect(page.locator('#in-share_encoding')).toHaveValue('base64');
  await expect(page.locator('#in-threshold')).toHaveValue('2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Recovered secret:\ncorrect horse', { timeout: 15_000 });
  await expect(out).toContainText('Share format:      leading-index');
  await expect(out).toContainText('Verification:      passed');
});

test('shamir-secret-recover covers trailing-index 0x11d, JSON, and verify off', async ({ page }) => {
  await page.goto('/tools/shamir-secret-recover/');
  await setValue(page, '#in-shares', TRAILING_11D);
  await page.selectOption('#in-share_format', 'trailing-index');
  await page.selectOption('#in-share_encoding', 'hex');
  await page.selectOption('#in-field_poly', '0x11d');
  await setValue(page, '#in-threshold', '3');
  await page.locator('#in-verify').uncheck();
  await page.selectOption('#in-secret_encoding', 'hex');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"field_poly": "0x11d"', { timeout: 15_000 });
  await expect(out).toContainText('"share_format": "trailing-index"');
  await expect(out).toContainText('"secret_hex": "7661756c742d6d61737465722d6b6579"');
  await expect(out).toContainText('"status": "off"');
});
