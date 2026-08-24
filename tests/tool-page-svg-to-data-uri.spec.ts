import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('svg-to-data-uri emits a URL-encoded data URI and adds xmlns', async ({ page }) => {
  await page.goto('/tools/svg-to-data-uri/');
  await setField(page, '#in-svg', '<svg viewBox="0 0 1 1"><rect fill="#fff"/></svg>');

  await expect(page.locator('#tool-output')).toHaveText(
    "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'%3E%3Crect fill='%23fff'/%3E%3C/svg%3E",
    { timeout: 15_000 },
  );
});

test('svg-to-data-uri deep-link renders a CSS background snippet', async ({ page }) => {
  const qs = new URLSearchParams({
    svg: '<svg viewBox="0 0 1 1"><rect fill="#fff"/></svg>',
    encoding: 'url',
    output: 'css',
    quotes: 'single',
    minify: 'true',
    add_xmlns: 'true',
  });
  await page.goto(`/tools/svg-to-data-uri/?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('css');
  await expect(page.locator('#in-minify')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    "background-image: url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'%3E%3Crect fill='%23fff'/%3E%3C/svg%3E\");",
    { timeout: 15_000 },
  );
});

test('svg-to-data-uri covers base64 and snippet enum choices', async ({ page }) => {
  await page.goto('/tools/svg-to-data-uri/');
  await setField(page, '#in-svg', '<svg/>');
  await page.selectOption('#in-encoding', 'base64');
  await page.selectOption('#in-output', 'img');

  await expect(page.locator('#tool-output')).toHaveText(
    '<img src="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=" alt="" />',
    { timeout: 15_000 },
  );

  await page.selectOption('#in-output', 'mask');
  await expect(page.locator('#tool-output')).toContainText('-webkit-mask-image: url("data:image/svg+xml;base64,', {
    timeout: 15_000,
  });
});

test('svg-to-data-uri honors non-default quote and checkbox states', async ({ page }) => {
  await page.goto('/tools/svg-to-data-uri/');
  await setField(page, '#in-svg', '<svg viewBox="0 0 1 1">\n  <text>a  b</text>\n</svg>');
  await page.selectOption('#in-quotes', 'encode');
  await page.uncheck('#in-minify');
  await page.uncheck('#in-add_xmlns');

  await expect(page.locator('#tool-output')).toHaveText(
    'data:image/svg+xml,%3Csvg viewBox=%220 0 1 1%22%3E%0A  %3Ctext%3Ea  b%3C/text%3E%0A%3C/svg%3E',
    { timeout: 15_000 },
  );
});

test('svg-to-data-uri reports compare output and invalid input errors', async ({ page }) => {
  await page.goto('/tools/svg-to-data-uri/');
  await setField(page, '#in-svg', '<svg/>');
  await page.selectOption('#in-output', 'compare');
  await expect(page.locator('#tool-output')).toContainText('URL-encoded :', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Base64      :', { timeout: 15_000 });

  await setField(page, '#in-svg', '<div>not svg</div>');
  await expect(page.locator('#tool-output')).toContainText('no root <svg> element found', {
    timeout: 15_000,
  });
});
