import { test, expect } from './fixtures';

// The code field is multiline (renders as <textarea>); set its value directly and
// dispatch the same 'input' event the driver listens to so newlines survive.
async function setCode(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-code').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('code-formatter pretty-prints JSON with exact indentation (auto-detect)', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, '{"name":"gizza","tags":["a","b"],"nested":{"x":1}}');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "gizza"', { timeout: 15_000 });
  // toContainText normalizes whitespace, so assert exact indentation via textContent.
  expect(await out.textContent()).toBe(
    '{\n  "name": "gizza",\n  "tags": [\n    "a",\n    "b"\n  ],\n  "nested": {\n    "x": 1\n  }\n}',
  );
});

test('code-formatter formats CSS with exact output', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, 'a{color:red;margin:0}');
  await page.selectOption('#in-language', 'css');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('color: red;', { timeout: 15_000 });
  expect(await out.textContent()).toBe('a {\n  color: red;\n  margin: 0;\n}\n');
});

test('code-formatter formats HTML with exact nesting', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, '<div><p>hi</p></div>');
  await page.selectOption('#in-language', 'html');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<div>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<div>\n  <p>\n    hi\n  </p>\n</div>\n');
});

test('code-formatter re-indents JavaScript', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, 'function f(){return 1;}');
  await page.selectOption('#in-language', 'javascript');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('function f()', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('function f() {');
  expect(text).toContain('\n  return 1;');
});

test('code-formatter deep-links JSON with tab indent', async ({ page }) => {
  const qs = new URLSearchParams({
    code: '{"a":1}',
    language: 'json',
    indent_char: 'tab',
  });
  await page.goto(`/tools/code-formatter/?${qs.toString()}`);

  await expect(page.locator('#in-code')).toHaveValue('{"a":1}');
  await expect(page.locator('#in-language')).toHaveValue('json');
  await expect(page.locator('#in-indent_char')).toHaveValue('tab');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"a": 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('{\n\t"a": 1\n}');
});

test('code-formatter honors a non-default indent width of 8 spaces', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, '{"a":1}');
  await page.selectOption('#in-language', 'json');
  await page.fill('#in-indent', '8');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"a": 1', { timeout: 15_000 });
  // 8 literal spaces before the key.
  expect(await out.textContent()).toBe('{\n        "a": 1\n}');
});

test('code-formatter covers every language option', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  const out = page.locator('#tool-output');

  // auto: HTML detected from the leading '<'.
  await setCode(page, '<span>x</span>');
  await page.selectOption('#in-language', 'auto');
  await expect(out).toContainText('<span>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<span>\n  x\n</span>\n');

  // explicit html
  await page.selectOption('#in-language', 'html');
  await expect(out).toContainText('<span>', { timeout: 15_000 });

  // explicit css
  await setCode(page, 'p{margin:0}');
  await page.selectOption('#in-language', 'css');
  await expect(out).toContainText('margin: 0;', { timeout: 15_000 });

  // explicit javascript
  await setCode(page, 'function f(){return 1;}');
  await page.selectOption('#in-language', 'javascript');
  await expect(out).toContainText('function f() {', { timeout: 15_000 });

  // explicit json
  await setCode(page, '{"k":true}');
  await page.selectOption('#in-language', 'json');
  await expect(out).toContainText('"k": true', { timeout: 15_000 });
});

test('code-formatter covers both indent_char options', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, '{"a":1}');
  await page.selectOption('#in-language', 'json');

  const out = page.locator('#tool-output');

  await page.selectOption('#in-indent_char', 'space');
  await expect(out).toContainText('"a": 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('{\n  "a": 1\n}');

  await page.selectOption('#in-indent_char', 'tab');
  await expect(out).toContainText('"a": 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('{\n\t"a": 1\n}');
});

test('code-formatter reports invalid JSON as an error', async ({ page }) => {
  await page.goto('/tools/code-formatter/');
  await setCode(page, '{not json}');
  await page.selectOption('#in-language', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('invalid JSON', { timeout: 15_000 });
  await expect(out).toHaveClass(/error/);
});
