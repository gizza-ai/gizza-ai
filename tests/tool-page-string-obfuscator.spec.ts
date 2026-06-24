import { test, expect } from './fixtures';

test('string-obfuscator masks the middle keeping ends', async ({ page }) => {
  await page.goto('/tools/string-obfuscator/');

  // Default mode = mask. Keep first 5 + last 4.
  await page.fill('#in-text', 'sk-1234567890abcdef');
  await page.fill('#in-keep_start', '5');
  await page.fill('#in-keep_end', '4');
  await expect(page.locator('#tool-output')).toHaveText('sk-12**********cdef', {
    timeout: 15000,
  });
});

test('string-obfuscator rot13 rotates letters', async ({ page }) => {
  await page.goto('/tools/string-obfuscator/');
  await page.fill('#in-text', 'Hello, World!');
  await page.selectOption('#in-mode', 'rot');
  // rot_n defaults to 13.
  await expect(page.locator('#tool-output')).toHaveText('Uryyb, Jbeyq!', {
    timeout: 15000,
  });
});

test('string-obfuscator leetspeak swaps letters for digits', async ({ page }) => {
  await page.goto('/tools/string-obfuscator/');
  await page.fill('#in-text', 'elite hacker');
  await page.selectOption('#in-mode', 'leetspeak');
  await expect(page.locator('#tool-output')).toHaveText('31173 h4ck3r', {
    timeout: 15000,
  });
});

test('string-obfuscator query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/string-obfuscator/?text=' +
      encodeURIComponent('Hello, World!') +
      '&mode=rot',
  );
  await expect(page.locator('#in-text')).toHaveValue('Hello, World!', {
    timeout: 15000,
  });
  await expect(page.locator('#in-mode')).toHaveValue('rot');
  await expect(page.locator('#tool-output')).toHaveText('Uryyb, Jbeyq!', {
    timeout: 15000,
  });
});
