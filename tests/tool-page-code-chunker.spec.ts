import { test, expect } from './fixtures';

const rustCode = 'fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n';

// /tools/code-chunker/ splits source into function-/class-aligned chunks in-browser.
test('code-chunker emits exact JSON records for Rust functions', async ({ page }) => {
  await page.goto('/tools/code-chunker/');
  await page.fill('#in-code', rustCode);
  await page.selectOption('#in-language', 'rust');
  await page.fill('#in-max_lines', '3');
  await page.selectOption('#in-format', 'json');

  const expected = JSON.stringify(
    [
      {
        end_line: 3,
        index: 0,
        kind: 'function',
        line_count: 3,
        name: 'add',
        oversize: false,
        start_line: 1,
        text: 'fn add(a: i32, b: i32) -> i32 {\n    a + b\n}',
      },
      {
        end_line: 7,
        index: 1,
        kind: 'function',
        line_count: 3,
        name: 'sub',
        oversize: false,
        start_line: 5,
        text: 'fn sub(a: i32, b: i32) -> i32 {\n    a - b\n}',
      },
    ],
    null,
    2,
  );
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
});

test('code-chunker renders text output for a Python deep link', async ({ page }) => {
  const code = 'class Greeter:\n    def hello(self, name):\n        return name\n\nx = 3\n';
  await page.goto(
    '/tools/code-chunker/?code=' +
      encodeURIComponent(code) +
      '&language=python&max_lines=3&format=text',
  );

  await expect(page.locator('#in-code')).toHaveValue(code, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    '===== chunk 0 — class Greeter (lines 1-3) =====\n' +
      'class Greeter:\n    def hello(self, name):\n        return name\n\n' +
      '===== chunk 1 — code (lines 5-5) =====\n' +
      'x = 3',
    { timeout: 15000 },
  );
});

test('code-chunker reports invalid max_lines', async ({ page }) => {
  await page.goto('/tools/code-chunker/');
  await page.fill('#in-code', 'fn a() {}');
  await page.selectOption('#in-language', 'rust');
  await page.fill('#in-max_lines', '0');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('max_lines must be between 1 and 100000', {
    timeout: 15000,
  });
});

test('code-chunker emits JSON Lines for JavaScript comments plus functions', async ({ page }) => {
  await page.goto('/tools/code-chunker/');
  await page.fill('#in-code', '// adds two numbers\nfunction add(a, b) {\n  return a + b;\n}\n');
  await page.selectOption('#in-language', 'javascript');
  await page.fill('#in-max_lines', '4');
  await page.selectOption('#in-format', 'jsonl');

  await expect(page.locator('#tool-output')).toHaveText(
    '{"end_line":4,"index":0,"kind":"function","line_count":4,"name":"add","oversize":false,"start_line":1,"text":"// adds two numbers\\nfunction add(a, b) {\\n  return a + b;\\n}"}',
    { timeout: 15000 },
  );
});
