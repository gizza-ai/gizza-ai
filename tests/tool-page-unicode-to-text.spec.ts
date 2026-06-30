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

test('unicode-to-text page decodes common unicode escapes', async ({ page }) => {
  await page.goto('/tools/unicode-to-text/');
  await fillText(page, '#in-text', String.raw`caf\u00e9 \u{1F600} U+2764`);
  await expect(page.locator('#tool-output')).toHaveText('café 😀 ❤', { timeout: 15000 });
});

test('unicode-to-text page decodes HTML numeric references', async ({ page }) => {
  await page.goto('/tools/unicode-to-text/');
  await fillText(page, '#in-text', '&#65;&#x42;&#128512;');
  await expect(page.locator('#tool-output')).toHaveText('AB😀', { timeout: 15000 });
});

test('unicode-to-text page combines surrogate pairs and preserves plain text', async ({ page }) => {
  await page.goto('/tools/unicode-to-text/');
  await fillText(page, '#in-text', String.raw`emoji=\uD83D\uDE00 line\nbreak`);
  await expect(page.locator('#tool-output')).toHaveText(String.raw`emoji=😀 line\nbreak`, {
    timeout: 15000,
  });
});

test('unicode-to-text query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/unicode-to-text/?text=' + encodeURIComponent(String.raw`caf\u00e9 U+1F600`));
  await expect(page.locator('#in-text')).toHaveValue(String.raw`caf\u00e9 U+1F600`, {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText('café 😀', { timeout: 15000 });
});
