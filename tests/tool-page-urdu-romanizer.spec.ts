import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const DEFAULT_INPUT = `یہ کتاب اچھی ہے۔
پاکستان ۲۰۲۶`;
const KEEP_INPUT = '۱۲۳ ہے، جی۔';

test('urdu-romanizer page emits exact default Roman Urdu', async ({ page }) => {
  await page.goto('/tools/urdu-romanizer/');
  await setField(page, '#in-input', DEFAULT_INPUT);

  await expect(page.locator('#tool-output')).toContainText('Yeh kitab achhi hai.', { timeout: 15_000 });
  expect(await outputText(page)).toBe('Yeh kitab achhi hai.\nPakistan 2026');
});

test('urdu-romanizer deep-link fills params and keeps script punctuation', async ({ page }) => {
  const params = new URLSearchParams({
    input: KEEP_INPUT,
    scheme: 'informal',
    short_vowels: 'insert-a',
    common_words: 'true',
    digits: 'keep',
    punctuation: 'keep',
    capitalization: 'none',
  });

  await page.goto(`/tools/urdu-romanizer/?${params.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(KEEP_INPUT, { timeout: 15_000 });
  await expect(page.locator('#in-digits')).toHaveValue('keep');
  await expect(page.locator('#in-punctuation')).toHaveValue('keep');
  await expect(page.locator('#in-capitalization')).toHaveValue('none');
  await expect(page.locator('#tool-output')).toContainText('۱۲۳ hai، ji۔', { timeout: 15_000 });
  expect(await outputText(page)).toBe('۱۲۳ hai، ji۔');
});

test('urdu-romanizer exercises enum choices and unchecked common words', async ({ page }) => {
  await page.goto('/tools/urdu-romanizer/');
  await setField(page, '#in-input', 'پاکستان');
  await page.selectOption('#in-scheme', 'informal');
  await page.selectOption('#in-short_vowels', 'insert-a');
  await page.uncheck('#in-common_words');
  await page.selectOption('#in-digits', 'latin');
  await page.selectOption('#in-punctuation', 'latin');
  await page.selectOption('#in-capitalization', 'none');

  await expect(page.locator('#tool-output')).toContainText('pakasatan', { timeout: 15_000 });
  expect(await outputText(page)).toBe('pakasatan');

  await setField(page, '#in-input', 'طٹت');
  await page.selectOption('#in-scheme', 'iso15919');
  await page.selectOption('#in-short_vowels', 'marks-only');
  await expect(page.locator('#tool-output')).toContainText('ṭṭt', { timeout: 15_000 });
});

test('urdu-romanizer shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/urdu-romanizer/');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool urdu-romanizer');
  expect(cli).toContain('یہ کتاب اچھی ہے');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
