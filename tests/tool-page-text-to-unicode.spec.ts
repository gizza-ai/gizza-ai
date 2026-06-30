import { test, expect } from './fixtures';

async function fillText(page: any, selector: string, value: string) {
  await page.$eval(
    selector,
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('text-to-unicode page renders an aligned code-point table', async ({ page }) => {
  await page.goto('/tools/text-to-unicode/');
  await fillText(page, '#in-text', 'A😀');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('U+0041', { timeout: 15000 });
  await expect(out).toContainText('LATIN CAPITAL LETTER A');
  await expect(out).toContainText('\\u0041');
  await expect(out).toContainText('U+1F600');
  await expect(out).toContainText('\\u{1F600}');
});

test('text-to-unicode page supports JSON output', async ({ page }) => {
  await page.goto('/tools/text-to-unicode/');
  await page.selectOption('#in-format', 'json');
  await fillText(page, '#in-text', 'A');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"codePoint": "U+0041"', { timeout: 15000 });
  await expect(out).toContainText('"decimal": 65');
});

test('text-to-unicode query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/text-to-unicode/?text=' + encodeURIComponent('Hi 😀') + '&format=json');
  await expect(page.locator('#in-text')).toHaveValue('Hi 😀', { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('U+1F600', { timeout: 15000 });
});
