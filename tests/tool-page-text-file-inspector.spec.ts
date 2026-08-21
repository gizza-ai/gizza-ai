import { test, expect } from './fixtures';

async function setText(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('text-file-inspector reports normalized pasted text and whitespace exactly', async ({ page }) => {
  await page.goto('/tools/text-file-inspector/');
  await setText(page, 'alpha\r\nbeta beta   \ngamma\rdelta');
  await page.selectOption('#in-output', 'report');
  await page.locator('#in-longest_lines').fill('3');
  await page.locator('#in-max_line_length').fill('8');
  await page.locator('#in-preview_lines').fill('4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Line endings  LF (Unix, macOS, Linux)', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'File          30 bytes (30 B)\n' +
      'Encoding      ASCII (via ascii)\n' +
      'BOM           none\n\n' +
      'Line endings  LF (Unix, macOS, Linux)\n' +
      '  CRLF        0 (0%)\n' +
      '  LF          3 (100%)\n' +
      '  CR          0 (0%)\n' +
      'Final newline no\n\n' +
      'Lines         4 (3 terminated, 1 unterminated)\n' +
      'Longest line  12 chars (line 2)\n' +
      'Average line  6.8 chars\n' +
      'Blank lines   0 empty, 0 whitespace-only\n' +
      'Trailing WS   1 line(s) — 2\n' +
      'Indentation   none\n' +
      'Characters    30 total, 0 non-ASCII, 0 control, 0 NUL\n\n' +
      'Over 8 chars  1 line(s) — 2\n\n' +
      'Longest 3 lines:\n' +
      '       2      12 chars  beta beta   \n' +
      '       1       5 chars  alpha\n' +
      '       3       5 chars  gamma\n\n' +
      'Preview (first 4 line(s); ␍ = CR, ␊ = LF, → = tab, ⏎̸ = no terminator):\n' +
      '       1 | alpha␊\n' +
      '       2 | beta beta   ␊\n' +
      '       3 | gamma␊\n' +
      '       4 | delta⏎̸\n\n' +
      'Notes:\n' +
      '  • No newline at end of file — POSIX tools (wc, diff, cat) and many linters expect a trailing newline.',
  );
});

test('text-file-inspector deep-link renders json output', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'name,value\nalpha,1\nbeta,2\n',
    output: 'json',
    longest_lines: '2',
    max_line_length: '0',
    preview_lines: '0',
  });
  await page.goto(`/tools/text-file-inspector/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue('name,value\nalpha,1\nbeta,2\n');
  await expect(page.locator('#in-output')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"dominant": "LF"', { timeout: 15_000 });
  const parsed = JSON.parse((await out.textContent()) ?? '{}');
  expect(parsed.bytes).toBe(26);
  expect(parsed.line_endings).toEqual({ cr: 0, crlf: 0, dominant: 'LF', lf: 3, mixed: false });
  expect(parsed.lines.total).toBe(3);
  expect(parsed.lines.final_newline).toBe(true);
  expect(parsed.lines.longest[0]).toEqual({ chars: 10, line: 1, terminator: 'LF', text: 'name,value' });
});
