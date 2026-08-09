import { test, expect } from './fixtures';

async function setInput(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('number-words-to-digits replaces numbers inside prose with exact output', async ({ page }) => {
  await page.goto('/tools/number-words-to-digits/');
  await setInput(page, 'We shipped twenty-five units and one hundred and two spares.');
  await expect(page.locator('#tool-output')).toHaveText('We shipped 25 units and 102 spares.', { timeout: 15000 });
});

test('number-words-to-digits deep-links value mode and comma separator', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'one million two hundred fifty thousand',
    mode: 'value',
    separator: 'comma',
    scale: 'short',
    ordinals: 'cardinal',
    fractions: 'true',
    digit_sequences: 'false',
  });
  await page.goto(`/tools/number-words-to-digits/?${qs.toString()}`);
  await expect(page.locator('#in-mode')).toHaveValue('value');
  await expect(page.locator('#in-separator')).toHaveValue('comma');
  await expect(page.locator('#tool-output')).toHaveText('1,250,000', { timeout: 15000 });
});

test('number-words-to-digits covers enum choices and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/number-words-to-digits/');

  await setInput(page, 'order twelve widgets and thirty-four bolts');
  await page.selectOption('#in-mode', 'extract');
  await expect(page.locator('#tool-output')).toHaveText('12\n34', { timeout: 15000 });

  await page.selectOption('#in-mode', 'value');
  await setInput(page, 'one million two hundred fifty thousand');
  await page.selectOption('#in-separator', 'space');
  await expect(page.locator('#tool-output')).toHaveText('1 250 000', { timeout: 15000 });
  await page.selectOption('#in-separator', 'underscore');
  await expect(page.locator('#tool-output')).toHaveText('1_250_000', { timeout: 15000 });

  await setInput(page, 'one billion');
  await page.selectOption('#in-separator', 'none');
  await page.selectOption('#in-scale', 'long');
  await expect(page.locator('#tool-output')).toHaveText('1000000000000', { timeout: 15000 });

  await page.selectOption('#in-mode', 'replace');
  await setInput(page, 'the twenty-first of June');
  await page.selectOption('#in-ordinals', 'suffix');
  await expect(page.locator('#tool-output')).toHaveText('the 21st of June', { timeout: 15000 });
  await page.selectOption('#in-ordinals', 'ignore');
  await expect(page.locator('#tool-output')).toHaveText('the 20-first of June', { timeout: 15000 });

  await setInput(page, 'one and a half');
  await page.selectOption('#in-ordinals', 'cardinal');
  await page.uncheck('#in-fractions');
  await expect(page.locator('#tool-output')).toHaveText('1 and a half', { timeout: 15000 });

  await setInput(page, 'Call nine one one');
  await page.check('#in-fractions');
  await page.check('#in-digit_sequences');
  await expect(page.locator('#tool-output')).toHaveText('Call 911', { timeout: 15000 });
});

test('number-words-to-digits accepts the exact input cap boundary', async ({ page }) => {
  await page.goto('/tools/number-words-to-digits/');
  const big = 'x'.repeat(200000);
  await setInput(page, big);
  await expect(page.locator('#tool-output')).toContainText('xxxx', { timeout: 15000 });
  const length = await page.locator('#tool-output').evaluate((el) => el.textContent?.length ?? 0);
  expect(length).toBe(200000);
});
