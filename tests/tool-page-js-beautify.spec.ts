import { test, expect } from './fixtures';

// /tools/js-beautify/ re-indents minified/obfuscated JavaScript in-browser (pure wasm).
test('js-beautify re-indents a minified function with 2-space indent', async ({ page }) => {
  await page.goto('/tools/js-beautify/');
  await page.fill('#in-code', 'function f(){return 1;}');
  await page.fill('#in-indent', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('function f() {', { timeout: 15000 });
  await expect(out).toContainText('  return 1;');
});

test('js-beautify respects a custom indent width', async ({ page }) => {
  await page.goto('/tools/js-beautify/');
  await page.fill('#in-code', 'if(a){b();}');
  await page.fill('#in-indent', '4');
  await expect(page.locator('#tool-output')).toContainText('    b();', { timeout: 15000 });
});

test('js-beautify formats a ternary with spaced colon', async ({ page }) => {
  await page.goto('/tools/js-beautify/');
  await page.fill('#in-code', 'var x=a?1:2;');
  await page.fill('#in-indent', '2');
  await expect(page.locator('#tool-output')).toContainText('a ? 1 : 2', { timeout: 15000 });
});

test('js-beautify indents with a tab when indent_char=tab', async ({ page }) => {
  await page.goto('/tools/js-beautify/');
  await page.fill('#in-code', 'function f(){return 1;}');
  await page.selectOption('#in-indent_char', 'tab');
  await expect(page.locator('#tool-output')).toContainText('\treturn 1;', { timeout: 15000 });
});
