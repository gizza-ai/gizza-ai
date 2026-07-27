import { test, expect } from './fixtures';

test('cookie-string-to-json page parses a header into a JSON object, values decoded', async ({ page }) => {
  await page.goto('/tools/cookie-string-to-json/');
  await page.fill('#in-cookie', 'sessionid=abc123; theme=dark; redirect=%2Faccount%2Fsettings');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('sessionid', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual({
    sessionid: 'abc123',
    theme: 'dark',
    redirect: '/account/settings',
  });
});

test('cookie-string-to-json deep link selects pairs shape and keeps duplicates in order', async ({ page }) => {
  const qs =
    '?cookie=' + encodeURIComponent('id=1; theme=dark; id=2') +
    '&output=pairs';
  await page.goto('/tools/cookie-string-to-json/' + qs);

  await expect(page.locator('#in-output')).toHaveValue('pairs', { timeout: 15_000 });

  const out = page.locator('#tool-output');
  await expect(out).toContainText('name', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual([
    { name: 'id', value: '1' },
    { name: 'theme', value: 'dark' },
    { name: 'id', value: '2' },
  ]);
});

test('cookie-string-to-json keeps values raw when URL-decode is turned off', async ({ page }) => {
  await page.goto('/tools/cookie-string-to-json/');
  await page.fill('#in-cookie', 'redirect=%2Fhome');
  await page.uncheck('#in-decode');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('redirect', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual({ redirect: '%2Fhome' });
});

test('cookie-string-to-json strips a pasted Cookie: header name and collapses duplicates to an array', async ({ page }) => {
  await page.goto('/tools/cookie-string-to-json/');
  await page.fill('#in-cookie', 'Cookie: id=1; id=2; id=3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('id', { timeout: 15_000 });
  expect(JSON.parse((await out.textContent()) ?? '')).toEqual({ id: ['1', '2', '3'] });
});
