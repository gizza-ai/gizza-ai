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
