import { test, expect } from './fixtures';

const ESC = '\x1b';

async function setText(page: any, value: string) {
  await page.$eval(
    '#in-text',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('ansi-log-renderer page renders colored HTML with inline styles', async ({ page }) => {
  await page.goto('/tools/ansi-log-renderer/');
  await setText(page, `${ESC}[1;32m✓ build passed${ESC}[0m\n${ESC}[31mERROR${ESC}[0m: missing <file>`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<pre style="background-color:#0c0c0c;color:#cccccc', { timeout: 15000 });
  await expect(out).toContainText('<span style="color:#00cd00;font-weight:bold">✓ build passed</span>');
  await expect(out).toContainText('<span style="color:#cd0000">ERROR</span>: missing &lt;file&gt;');
});

test('ansi-log-renderer page strips to plain text', async ({ page }) => {
  await page.goto('/tools/ansi-log-renderer/');
  await page.selectOption('#in-output', 'text');
  await setText(page, `${ESC}[2J${ESC}[H${ESC}[33mwarn${ESC}[0m\nplain`);

  await expect(page.locator('#tool-output')).toHaveText('warn\nplain', { timeout: 15000 });
});

test('ansi-log-renderer query params prefill enum controls and compute classes mode', async ({ page }) => {
  const input = `${ESC}[38;5;196mred${ESC}[0m and ${ESC}[4munderlined${ESC}[0m`;
  await page.goto(
    '/tools/ansi-log-renderer/?text=' +
      encodeURIComponent(input) +
      '&output=html&theme=light&styles=classes',
  );

  await expect(page.locator('#in-text')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('html');
  await expect(page.locator('#in-theme')).toHaveValue('light');
  await expect(page.locator('#in-styles')).toHaveValue('classes');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<style>', { timeout: 15000 });
  await expect(out).toContainText('<pre class="ansi ansi--light">');
  await expect(out).toContainText('<span style="color:#ff0000">red</span>');
  await expect(out).toContainText('<span class="ansi-underline">underlined</span>');
});
