import { test, expect } from './fixtures';

test('change-case page upper/lower/title', async ({ page }) => {
  await page.goto('/tools/change-case/');

  // Default mode is 'upper' (the first <select> option).
  await page.fill('#in-text', 'the quick brown fox');
  await expect(page.locator('#tool-output')).toHaveText('THE QUICK BROWN FOX', {
    timeout: 15000,
  });

  // Title case.
  await page.selectOption('#in-mode', 'title');
  await expect(page.locator('#tool-output')).toHaveText('The Quick Brown Fox', {
    timeout: 15000,
  });

  // lowercase.
  await page.fill('#in-text', 'Hello WORLD');
  await page.selectOption('#in-mode', 'lower');
  await expect(page.locator('#tool-output')).toHaveText('hello world', {
    timeout: 15000,
  });
});

test('change-case page identifier cases', async ({ page }) => {
  await page.goto('/tools/change-case/');

  // snake_case re-tokenizes an existing camelCase identifier.
  await page.fill('#in-text', 'helloWorld');
  await page.selectOption('#in-mode', 'snake');
  await expect(page.locator('#tool-output')).toHaveText('hello_world', {
    timeout: 15000,
  });

  // CONSTANT_CASE from spaced words.
  await page.fill('#in-text', 'My API Key');
  await page.selectOption('#in-mode', 'constant');
  await expect(page.locator('#tool-output')).toHaveText('MY_API_KEY', {
    timeout: 15000,
  });
});
