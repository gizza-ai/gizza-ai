import { test, expect } from './fixtures';

// /tools/markdown-lint/ lints + auto-fixes Markdown in-browser (pure wasm).
// markdown is a multiline <textarea>; mode is a <select> (check | fix).

test('markdown-lint check reports issues', async ({ page }) => {
  await page.goto('/tools/markdown-lint/');
  await page.fill('#in-markdown', '#Heading\n\n* a\n- b\n');
  await page.selectOption('#in-mode', 'check');
  const out = page.locator('#tool-output');
  // MD018 (no space after #) and MD004 (mixed list markers) should be reported.
  await expect(out).toContainText('MD018', { timeout: 15000 });
  await expect(out).toContainText('MD004');
  await expect(out).toContainText('issue(s) found');
});

test('markdown-lint fix returns corrected markdown', async ({ page }) => {
  await page.goto('/tools/markdown-lint/');
  await page.fill('#in-markdown', '#Heading\n\n* a\n- b\n');
  await page.selectOption('#in-mode', 'fix');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Heading', { timeout: 15000 });
  const text = (await out.textContent())!;
  // Heading hash spacing fixed and list markers normalized to '*'.
  expect(text).toContain('# Heading');
  expect(text).toContain('* a');
  expect(text).toContain('* b');
  expect(text).not.toContain('- b');
});

test('markdown-lint reports a clean document', async ({ page }) => {
  await page.goto('/tools/markdown-lint/');
  await page.fill('#in-markdown', '# Title\n\nSome clean text.\n');
  await page.selectOption('#in-mode', 'check');
  await expect(page.locator('#tool-output')).toContainText('No issues', { timeout: 15000 });
});
