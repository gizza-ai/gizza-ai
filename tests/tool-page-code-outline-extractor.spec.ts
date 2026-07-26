import { test, expect } from './fixtures';

const pythonCode = 'class Greeter:\n    def hi(self):\n        return 1\n\ndef main():\n    pass\n';

test('code-outline-extractor emits exact tree output for Python defaults', async ({ page }) => {
  await page.goto('/tools/code-outline-extractor/');
  await page.fill('#in-code', pythonCode);
  await page.selectOption('#in-language', 'python');
  await page.selectOption('#in-format', 'tree');

  await expect(page.locator('#tool-output')).toHaveText(
    'class Greeter  [L1]\n  method hi  [L2]\nfunction main  [L5]',
    { timeout: 15000 },
  );
});

test('code-outline-extractor supports deep-link params and markdown output without line numbers', async ({ page }) => {
  const jsCode = 'class Calc {\n  add(a, b) {\n    return a + b;\n  }\n}\n';
  await page.goto(
    '/tools/code-outline-extractor/?code=' +
      encodeURIComponent(jsCode) +
      '&language=javascript&format=markdown&signatures=&line_numbers=',
  );

  await expect(page.locator('#in-code')).toHaveValue(jsCode, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    '- **class** `Calc`\n  - **method** `add`',
    { timeout: 15000 },
  );
});

test('code-outline-extractor renders signatures and JSON for explicit Rust input', async ({ page }) => {
  const rustCode = 'struct Point { x: i32 }\n\nimpl Point {\n    fn new() -> Self {\n        Point { x: 0 }\n    }\n}\n';
  await page.goto('/tools/code-outline-extractor/');
  await page.fill('#in-code', rustCode);
  await page.selectOption('#in-language', 'rust');
  await page.selectOption('#in-format', 'json');
  await page.check('#in-signatures');

  const expected = JSON.stringify(
    [
      {
        children: [],
        kind: 'struct',
        line: 1,
        name: 'Point',
        signature: 'struct Point { x: i32 }',
      },
      {
        children: [
          {
            children: [],
            kind: 'method',
            line: 4,
            name: 'new',
            signature: 'fn new() -> Self',
          },
        ],
        kind: 'impl',
        line: 3,
        name: 'Point',
        signature: 'impl Point',
      },
    ],
    null,
    2,
  );
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
});
