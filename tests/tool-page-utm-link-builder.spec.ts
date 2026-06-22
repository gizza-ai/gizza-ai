import { test, expect } from './fixtures';

test('utm-link-builder page builds a tagged URL', async ({ page }) => {
  await page.goto('/tools/utm-link-builder/');

  await page.fill('#in-url', 'https://example.com/page');
  await page.fill('#in-source', 'google');
  await page.fill('#in-medium', 'cpc');
  await page.fill('#in-campaign', 'spring sale');

  await expect(page.locator('#tool-output')).toHaveText(
    'https://example.com/page?utm_source=google&utm_medium=cpc&utm_campaign=spring+sale',
    { timeout: 15000 },
  );
});

test('utm-link-builder encodes optional term/content and preserves existing query', async ({
  page,
}) => {
  await page.goto('/tools/utm-link-builder/');

  await page.fill('#in-url', 'https://x.io/p?id=7#sec');
  await page.fill('#in-source', 'google');
  await page.fill('#in-medium', 'cpc');
  await page.fill('#in-campaign', 'promo');
  await page.fill('#in-term', 'shoes & socks');
  await page.fill('#in-content', 'ad/1');

  await expect(page.locator('#tool-output')).toHaveText(
    'https://x.io/p?id=7&utm_source=google&utm_medium=cpc&utm_campaign=promo&utm_term=shoes+%26+socks&utm_content=ad%2F1#sec',
    { timeout: 15000 },
  );
});

test('utm-link-builder lowercase checkbox normalizes values', async ({ page }) => {
  await page.goto('/tools/utm-link-builder/');

  await page.fill('#in-url', 'https://x.io');
  await page.fill('#in-source', 'Google');
  await page.fill('#in-medium', 'Email');
  await page.fill('#in-campaign', 'Launch');
  await page.check('#in-lowercase');

  await expect(page.locator('#tool-output')).toHaveText(
    'https://x.io?utm_source=google&utm_medium=email&utm_campaign=launch',
    { timeout: 15000 },
  );
});

test('utm-link-builder appends GA4 utm_id and platform fields', async ({ page }) => {
  await page.goto('/tools/utm-link-builder/');

  await page.fill('#in-url', 'https://example.com');
  await page.fill('#in-source', 'google');
  await page.fill('#in-medium', 'cpc');
  await page.fill('#in-campaign', 'promo');
  await page.fill('#in-id', 'abc.123');
  await page.fill('#in-source_platform', 'Google Ads');

  await expect(page.locator('#tool-output')).toHaveText(
    'https://example.com?utm_id=abc.123&utm_source=google&utm_medium=cpc&utm_campaign=promo&utm_source_platform=Google+Ads',
    { timeout: 15000 },
  );
});

test('utm-link-builder query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto(
    '/tools/utm-link-builder/?url=' +
      encodeURIComponent('https://example.com') +
      '&source=newsletter&medium=email&campaign=june',
  );
  await expect(page.locator('#in-url')).toHaveValue('https://example.com', {
    timeout: 15000,
  });
  await expect(page.locator('#in-source')).toHaveValue('newsletter');
  await expect(page.locator('#tool-output')).toHaveText(
    'https://example.com?utm_source=newsletter&utm_medium=email&utm_campaign=june',
    { timeout: 15000 },
  );
});
