import { test, expect } from './fixtures';

test('url-encode page', async ({ page }) => {
  await page.goto('/tools/url-encode/');

  // Default mode (blank) = encode, default target (blank) = component.
  await page.fill('#in-text', 'name=John Doe&city=São Paulo');
  await expect(page.locator('#tool-output')).toHaveText(
    'name%3DJohn%20Doe%26city%3DS%C3%A3o%20Paulo',
    { timeout: 15000 },
  );

  // Decode reverses it (target ignored on decode).
  await page.fill('#in-text', 'S%C3%A3o%20Paulo');
  await page.fill('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('São Paulo', {
    timeout: 15000,
  });
});

test('url-encode form target encodes space as plus', async ({ page }) => {
  await page.goto('/tools/url-encode/');
  await page.fill('#in-text', 'a b+c');
  await page.fill('#in-target', 'form');
  // space -> '+', literal '+' -> '%2B'.
  await expect(page.locator('#tool-output')).toHaveText('a+b%2Bc', {
    timeout: 15000,
  });
});

test('url-encode per-line converts each line independently', async ({ page }) => {
  await page.goto('/tools/url-encode/');
  await page.fill('#in-text', 'a b\nc&d');
  await page.fill('#in-per_line', 'true');
  // Newlines are preserved; each line is encoded on its own.
  await expect(page.locator('#tool-output')).toHaveText('a%20b\nc%26d', {
    timeout: 15000,
  });
});

test('url-encode repeat un-nests double-encoded input', async ({ page }) => {
  await page.goto('/tools/url-encode/');
  await page.fill('#in-text', 'a%2520b');
  await page.fill('#in-mode', 'decode');
  await page.fill('#in-repeat', '2');
  await expect(page.locator('#tool-output')).toHaveText('a b', {
    timeout: 15000,
  });
});

test('url-encode query-param deep-link prefills new fields', async ({ page }) => {
  // ?text=...&target=form&per_line=... should prefill and compute.
  await page.goto(
    '/tools/url-encode/?text=' + encodeURIComponent('a b') + '&target=form',
  );
  await expect(page.locator('#in-text')).toHaveValue('a b', { timeout: 15000 });
  await expect(page.locator('#in-target')).toHaveValue('form');
  await expect(page.locator('#tool-output')).toHaveText('a+b', {
    timeout: 15000,
  });
});
