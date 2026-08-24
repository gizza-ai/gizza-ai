import { test, expect } from './fixtures';

const bundle = [
  '=== src/a.js ===',
  'const oldName = 1;',
  'use(oldName);',
  '=== src/b.js ===',
  'const other = 2;',
  '',
].join('\n');

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('regex-codemod page renders an exact unified diff', async ({ page }) => {
  await page.goto('/tools/regex-codemod/');
  await page.fill('#in-text', bundle);
  await page.fill('#in-pattern', '\\boldName\\b');
  await page.fill('#in-replacement', 'newName');

  await expect(page.locator('#tool-output')).toContainText('2 replacements', { timeout: 15000 });
  expect(await output(page)).toBe(
    '# 2 replacements in 1 of 2 files\n' +
      '--- a/src/a.js\n' +
      '+++ b/src/a.js\n' +
      '@@ -1,2 +1,2 @@\n' +
      '-const oldName = 1;\n' +
      '-use(oldName);\n' +
      '+const newName = 1;\n' +
      '+use(newName);'
  );
});

test('regex-codemod deep-link pre-fills capture-group replacement and full output', async ({ page }) => {
  await page.goto(
    '/tools/regex-codemod/?' +
      new URLSearchParams({
        text: '==> dates.txt <==\n12/31/2026\n',
        pattern: '(\\d{2})/(\\d{2})/(\\d{4})',
        replacement: '$3-$1-$2',
        file_marker: 'arrow',
        output: 'full',
      }).toString()
  );

  await expect(page.locator('#in-text')).toHaveValue('==> dates.txt <==\n12/31/2026\n', {
    timeout: 15000,
  });
  await expect(page.locator('#in-file_marker')).toHaveValue('arrow');
  await expect(page.locator('#in-output')).toHaveValue('full');
  await expect(page.locator('#tool-output')).toHaveText('==> dates.txt <==\n2026-12-31', {
    timeout: 15000,
  });
});

test('regex-codemod supports custom marker enum, json output, and unchanged file reporting', async ({ page }) => {
  await page.goto('/tools/regex-codemod/');
  await page.fill('#in-text', '@@@ a.txt\nx\n@@@ b.txt\ny\n');
  await page.fill('#in-pattern', 'x');
  await page.fill('#in-replacement', 'z');
  await page.selectOption('#in-file_marker', 'custom');
  await page.fill('#in-marker_regex', '^@@@ (\\S+)$');
  await page.selectOption('#in-output', 'json');
  await page.check('#in-include_unchanged');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"files_total": 2', { timeout: 15000 });
  await expect(out).toContainText('"path": "a.txt"');
  await expect(out).toContainText('"path": "b.txt"');
  await expect(out).toContainText('"changed": false');
});

test('regex-codemod replace_all can be turned off', async ({ page }) => {
  await page.goto('/tools/regex-codemod/');
  await page.fill('#in-text', 'old old old\n');
  await page.fill('#in-pattern', 'old');
  await page.fill('#in-replacement', 'new');
  await page.selectOption('#in-file_marker', 'none');
  await page.selectOption('#in-output', 'full');
  await page.uncheck('#in-replace_all');

  await expect(page.locator('#tool-output')).toHaveText('new old old', { timeout: 15000 });
});

test('regex-codemod accepts the documented 1,000,000 character cap', async ({ page }) => {
  await page.goto('/tools/regex-codemod/');
  const atCap = 'x'.repeat(1_000_000);
  await page.locator('#in-text').evaluate((el, value) => {
    (el as HTMLTextAreaElement).value = value;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, atCap);
  await page.fill('#in-pattern', 'z');
  await page.selectOption('#in-file_marker', 'none');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toContainText('"files_total": 1', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"replacements": 0');
});
