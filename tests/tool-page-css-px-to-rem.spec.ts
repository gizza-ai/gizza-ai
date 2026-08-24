import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('css-px-to-rem converts a stylesheet with default root size', async ({ page }) => {
  await page.goto('/tools/css-px-to-rem/');
  await setField(page, '#in-css', '.btn{font-size:24px;padding:8px 0px}');

  await expect(page.locator('#tool-output')).toHaveText(
    '.btn{font-size:1.5rem;padding:0.5rem 0}',
    { timeout: 15_000 },
  );
});

test('css-px-to-rem deep-link converts media queries and fallback declarations', async ({ page }) => {
  const qs = new URLSearchParams({
    css: '@media (min-width: 640px) { .btn { width: 32px; } }',
    direction: 'px-to-rem',
    root_font_size: '16',
    precision: '5',
    properties: '*',
    min_pixel_value: '0',
    media_queries: 'true',
    ignore_selectors: '',
    keep_fallback: 'true',
    unitless_zero: 'true',
  });
  await page.goto(`/tools/css-px-to-rem/?${qs.toString()}`);

  await expect(page.locator('#in-media_queries')).toBeChecked();
  await expect(page.locator('#in-keep_fallback')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    '@media (min-width: 40rem) { .btn { width: 32px; width: 2rem; } }',
    { timeout: 15_000 },
  );
});

test('css-px-to-rem covers reverse direction and 62.5 percent root', async ({ page }) => {
  await page.goto('/tools/css-px-to-rem/');
  await setField(page, '#in-css', '.btn{font-size:1.5rem;padding:0.8rem}');
  await page.selectOption('#in-direction', 'rem-to-px');
  await setField(page, '#in-root_font_size', '10');

  await expect(page.locator('#tool-output')).toHaveText('.btn{font-size:15px;padding:8px}', {
    timeout: 15_000,
  });
});

test('css-px-to-rem honors property filters, hairline threshold, and unitless-zero toggle', async ({
  page,
}) => {
  await page.goto('/tools/css-px-to-rem/');
  await setField(page, '#in-css', '.card{border:1px solid #eee;padding:24px;margin:0px}');
  await setField(page, '#in-properties', '*,!border*');
  await page.uncheck('#in-unitless_zero');

  await expect(page.locator('#tool-output')).toHaveText(
    '.card{border:1px solid #eee;padding:1.5rem;margin:0rem}',
    { timeout: 15_000 },
  );

  await setField(page, '#in-properties', '*');
  await setField(page, '#in-min_pixel_value', '2');
  await page.check('#in-unitless_zero');
  await expect(page.locator('#tool-output')).toHaveText(
    '.card{border:1px solid #eee;padding:1.5rem;margin:0px}',
    { timeout: 15_000 },
  );
});

test('css-px-to-rem enforces root and precision boundaries', async ({ page }) => {
  await page.goto('/tools/css-px-to-rem/');
  await setField(page, '#in-css', '.x{width:16px}');
  await setField(page, '#in-root_font_size', '0');
  await expect(page.locator('#tool-output')).toContainText('invalid root_font_size', {
    timeout: 15_000,
  });

  await setField(page, '#in-root_font_size', '16');
  await setField(page, '#in-precision', '11');
  await expect(page.locator('#tool-output')).toContainText('invalid precision', {
    timeout: 15_000,
  });
});
