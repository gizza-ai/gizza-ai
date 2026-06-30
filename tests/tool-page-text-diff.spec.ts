import { test, expect } from './fixtures';

const left = 'alpha\nbeta\ngamma\n';
const right = 'alpha\nBETA\ngamma\ndelta\n';

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

test('text-diff page renders a unified diff', async ({ page }) => {
  await page.goto('/tools/text-diff/');
  await fillText(page, '#in-left', left);
  await fillText(page, '#in-right', right);
  await page.fill('#in-context', '1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('--- left', { timeout: 15000 });
  await expect(out).toContainText('-beta');
  await expect(out).toContainText('+BETA');
  await expect(out).toContainText('+delta');
});

test('text-diff page supports JSON output and ignore flags', async ({ page }) => {
  await page.goto('/tools/text-diff/');
  await page.selectOption('#in-format', 'json');
  await page.check('#in-ignore_case');
  await page.check('#in-ignore_whitespace');
  await fillText(page, '#in-left', 'Hello   World');
  await fillText(page, '#in-right', 'hello world');
  await expect(page.locator('#tool-output')).toContainText('"equal": true', { timeout: 15000 });
});
