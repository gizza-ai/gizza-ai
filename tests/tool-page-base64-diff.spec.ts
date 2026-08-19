import { test, expect } from './fixtures';

test('base64-diff page reports the decoded byte that changed', async ({ page }) => {
  await page.goto('/tools/base64-diff/');
  await page.fill('#in-left', 'SGVsbG8gd29ybGQh');
  await page.fill('#in-right', 'SGVsbG8gV29ybGQh');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toHaveText(
    'Payloads differ: both 12 bytes. First difference at offset 0x0006 (6).\n' +
      '1 byte differs across 1 range.\n' +
      '@ 0x0006 (1 byte) changed: 77 |w| -> 57 |W|',
    { timeout: 15_000 }
  );
});

test('base64-diff deep-link uses shift alignment for one inserted byte', async ({ page }) => {
  const qs = new URLSearchParams({
    left: 'SGVsbG8gd29ybGQh',
    right: 'SGVsbG8sIHdvcmxkIQ==',
    alphabet: 'auto',
    strict: 'false',
    align: 'shift',
    output: 'summary',
    bytes_per_row: '8',
    context_rows: '2',
  });

  await page.goto(`/tools/base64-diff/?${qs.toString()}`);
  await expect(page.locator('#in-left')).toHaveValue('SGVsbG8gd29ybGQh', { timeout: 15_000 });
  await expect(page.locator('#in-align')).toHaveValue('shift');
  await expect(page.locator('#tool-output')).toContainText('Payloads differ: left 12 bytes, right 13 bytes (+1).');
  await expect(page.locator('#tool-output')).toContainText('@ 0x0005 (1 byte) added on the right: 2c |,|');
});
