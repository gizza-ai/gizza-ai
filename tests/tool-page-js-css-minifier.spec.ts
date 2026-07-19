import { test, expect } from './fixtures';

// /tools/js-css-minifier/ minifies JavaScript OR CSS in-browser (pure wasm).
test('js-css-minifier minifies CSS with a size report by default', async ({ page }) => {
  await page.goto('/tools/js-css-minifier/');
  await page.fill('#in-code', 'body {\n  margin: 0;\n  color: red;\n}');
  const out = page.locator('#tool-output');
  // "Prepend size report" is on by default → banner comment then minified CSS.
  await expect(out).toContainText('/* CSS:', { timeout: 15000 });
  await expect(out).toContainText('body{margin:0;color:red}');
});

test('js-css-minifier minifies JavaScript when Language forced to JS', async ({ page }) => {
  await page.goto('/tools/js-css-minifier/');
  await page.selectOption('#in-language', 'js');
  await page.uncheck('#in-report');
  await page.fill('#in-code', 'function  f ( ) {\n  return 1 ;\n}');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('function f(){return 1;}', { timeout: 15000 });
});

test('js-css-minifier keeps a /*! license banner while stripping comments', async ({ page }) => {
  await page.goto('/tools/js-css-minifier/');
  await page.selectOption('#in-language', 'css');
  await page.uncheck('#in-report');
  await page.fill('#in-code', '/*! keep me */\n/* drop me */\na{color:red}');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('/*! keep me */a{color:red}', { timeout: 15000 });
});

test('js-css-minifier page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/js-css-minifier/?code=.a%20%7B%20color%3A%20red%3B%20%7D&language=css&report=false');
  await expect(page.locator('#in-code')).toHaveValue('.a { color: red; }');
  await expect(page.locator('#in-language')).toHaveValue('css');
  await expect(page.locator('#tool-output')).toHaveText('.a{color:red}', { timeout: 15000 });
});
