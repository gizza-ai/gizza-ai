import { test, expect } from './fixtures';

// Exact-output assertions. The page writes String(value) into #tool-output
// verbatim, so the digest is byte-identical to the CLI. Multi-line results are
// compared against #tool-output.textContent (toHaveText normalizes whitespace),
// built with arrays joined by '\n' so indented tree lines are unambiguous.

test('markdown digest (default) — tree, then fenced files in input order', async ({ page }) => {
  await page.goto('/tools/files-to-prompt/');
  await page.fill('#in-files', '=== src/main.rs\nfn main() {}\n\n=== README.md\n# Title');
  await expect(page.locator('#tool-output')).toHaveText(/Directory structure/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    [
      'Directory structure:',
      '├── README.md',
      '└── src',
      '    └── main.rs',
      '',
      '## src/main.rs',
      '```rust',
      'fn main() {}',
      '```',
      '',
      '## README.md',
      '```markdown',
      '# Title',
      '```',
      '',
      '2 files, 137 characters, ~35 tokens (estimate)',
    ].join('\n'),
  );
});

test('xml format (select) with directory tree off', async ({ page }) => {
  await page.goto('/tools/files-to-prompt/');
  await page.fill('#in-files', '=== a.txt\nhello');
  await page.selectOption('#in-format', 'xml');
  await page.uncheck('#in-include_tree');
  await expect(page.locator('#tool-output')).toHaveText(/<documents>/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    [
      '<documents>',
      '<document index="1">',
      '<source>a.txt</source>',
      '<document_contents>',
      'hello',
      '</document_contents>',
      '</document>',
      '</documents>',
      '',
      '1 file, 127 characters, ~32 tokens (estimate)',
    ].join('\n'),
  );
});

test('plain format + line numbers (non-default checkbox on)', async ({ page }) => {
  await page.goto('/tools/files-to-prompt/');
  await page.fill('#in-files', '=== app.js\nconst x = 1\nconsole.log(x)');
  await page.selectOption('#in-format', 'plain');
  await page.check('#in-line_numbers');
  await page.uncheck('#in-include_tree');
  await expect(page.locator('#tool-output')).toHaveText(/console\.log/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    [
      'app.js',
      '---',
      '1  const x = 1',
      '2  console.log(x)',
      '---',
      '',
      '1 file, 47 characters, ~12 tokens (estimate)',
    ].join('\n'),
  );
});

test('custom separator (>>>) parses headers', async ({ page }) => {
  await page.goto('/tools/files-to-prompt/');
  await page.fill('#in-files', '>>> a.txt\nhi');
  await page.selectOption('#in-format', 'plain');
  await page.fill('#in-separator', '>>>');
  await page.uncheck('#in-include_tree');
  await expect(page.locator('#tool-output')).toHaveText(/a\.txt/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    ['a.txt', '---', 'hi', '---', '', '1 file, 16 characters, ~4 tokens (estimate)'].join('\n'),
  );
});

test('deep-link ?files=…&format=plain pre-fills and auto-runs', async ({ page }) => {
  const files = encodeURIComponent('=== a\nhi');
  await page.goto(`/tools/files-to-prompt/?files=${files}&format=plain`);
  await expect(page.locator('#tool-output')).toHaveText(/Directory structure/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    [
      'Directory structure:',
      '└── a',
      '',
      'a',
      '---',
      'hi',
      '---',
      '',
      '1 file, 40 characters, ~10 tokens (estimate)',
    ].join('\n'),
  );
});
