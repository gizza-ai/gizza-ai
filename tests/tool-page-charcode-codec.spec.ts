import { test, expect } from './fixtures';

async function setInput(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('charcode-codec encodes Unicode scalar values exactly', async ({ page }) => {
  await page.goto('/tools/charcode-codec/');
  await setInput(page, 'Hi 😀');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('72 105 32 128512', { timeout: 15_000 });
});

test('charcode-codec decodes UTF-8 hex bytes back to text', async ({ page }) => {
  await page.goto('/tools/charcode-codec/');
  await setInput(page, '48 65 6c 6c 6f');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-base', 'hex');
  await page.selectOption('#in-scope', 'utf8-bytes');

  await expect(page.locator('#tool-output')).toHaveText('Hello', { timeout: 15_000 });
});

test('charcode-codec covers enum choices and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/charcode-codec/');
  await setInput(page, 'Hi');
  await page.selectOption('#in-base', 'bin');
  await page.selectOption('#in-scope', 'ascii');
  await page.selectOption('#in-delimiter', 'newline');
  await page.selectOption('#in-prefix', 'none');
  await page.selectOption('#in-padding', 'fixed');
  await page.uncheck('#in-uppercase');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('1001000\n1101001', { timeout: 15_000 });
});

test('charcode-codec emits JSON output', async ({ page }) => {
  await page.goto('/tools/charcode-codec/');
  await setInput(page, 'Hi');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"codes": [72, 105]', { timeout: 15_000 });
  const json = JSON.parse((await out.textContent()) ?? '{}');
  expect(json).toMatchObject({ mode: 'encode', base: 'dec', scope: 'unicode-scalar', count: 2 });
  expect(json.output).toBe('72 105');
});

test('charcode-codec deep-links decode mode', async ({ page }) => {
  const qs = new URLSearchParams({ input: 'U+0048, U+0069', mode: 'decode', base: 'hex', scope: 'unicode-scalar' });
  await page.goto(`/tools/charcode-codec/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue('U+0048, U+0069');
  await expect(page.locator('#in-mode')).toHaveValue('decode');
  await expect(page.locator('#in-base')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toHaveText('Hi', { timeout: 15_000 });
});
