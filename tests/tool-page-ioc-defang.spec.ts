import { test, expect } from './fixtures';

test('ioc-defang page defangs a url', async ({ page }) => {
  await page.goto('/tools/ioc-defang/');

  // Default action is 'defang', default style 'square'.
  await page.fill('#in-text', 'http://evil.example.com/x');
  await expect(page.locator('#tool-output')).toHaveText(
    'hxxp[://]evil[.]example[.]com/x',
    { timeout: 15000 },
  );

  // Round bracket style on an email.
  await page.fill('#in-text', 'bad.actor@evil.com');
  await page.selectOption('#in-style', 'round');
  await expect(page.locator('#tool-output')).toHaveText(
    'bad(.)actor(at)evil(.)com',
    { timeout: 15000 },
  );

  // Dot (spelled-out) style.
  await page.fill('#in-text', 'a.b@c.d');
  await page.selectOption('#in-style', 'dot');
  await expect(page.locator('#tool-output')).toHaveText('a[dot]b[at]c[dot]d', {
    timeout: 15000,
  });
});

test('ioc-defang page refangs a blob', async ({ page }) => {
  await page.goto('/tools/ioc-defang/');

  await page.fill('#in-text', 'hxxps[://]10[.]0[.]0[.]1');
  await page.selectOption('#in-mode', 'refang');
  await expect(page.locator('#tool-output')).toHaveText('https://10.0.0.1', {
    timeout: 15000,
  });
});
