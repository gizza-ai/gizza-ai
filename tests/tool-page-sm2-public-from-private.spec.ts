import { test, expect } from './fixtures';

const D = '3945208F7B2144B13F36E38AC6D39F95889393692860B51A42FB81EF4DF7C5B8';
const X = '09f9df311e5421a150dd7d161e4bc5c672179fad1833fc076bb08ff356f35020';
const Y = 'ccea490ce26775a52dc6ea718cc1aa600aed05fbf35e084a6632f6072da9ad13';
const UNCOMPRESSED = `04${X}${Y}`;

test('sm2-public-from-private derives standard vector', async ({ page }) => {
  await page.goto('/tools/sm2-public-from-private/');
  await page.fill('#in-private_key', D);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('-----BEGIN PUBLIC KEY-----', { timeout: 15000 });
  await expect(out).toContainText(UNCOMPRESSED);
  await expect(out).toContainText('Curve: sm2p256v1');
});

test('sm2-public-from-private compressed output', async ({ page }) => {
  await page.goto('/tools/sm2-public-from-private/');
  await page.fill('#in-private_key', D);
  await page.selectOption('#in-output_format', 'compressed');
  await expect(page.locator('#tool-output')).toContainText(/^0[23][0-9a-f]{64}$/m, {
    timeout: 15000,
  });
});

test('sm2-public-from-private query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/sm2-public-from-private/?private_key=' +
      encodeURIComponent(D) +
      '&input_format=hex&output_format=uncompressed',
  );
  await expect(page.locator('#in-private_key')).toHaveValue(D, { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-output_format')).toHaveValue('uncompressed');
  await expect(page.locator('#tool-output')).toContainText(UNCOMPRESSED, { timeout: 15000 });
});
