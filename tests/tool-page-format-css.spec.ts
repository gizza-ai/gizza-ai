import { test, expect } from './fixtures';

async function setCss(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('format-css beautifies minified CSS with exact output', async ({ page }) => {
  await page.goto('/tools/format-css/');
  await setCss(page, 'a{color:red;margin:0}');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('color: red;', { timeout: 15_000 });
  expect(await out.textContent()).toBe('a {\n  color: red;\n  margin: 0;\n}\n');
});

test('format-css honors grouped sorting and uppercase hex', async ({ page }) => {
  await page.goto('/tools/format-css/');
  await setCss(page, 'a{color:#abcdef;position:absolute;width:1px}');
  await page.selectOption('#in-sort', 'grouped');
  await page.check('#in-uppercase_hex');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('position: absolute;', { timeout: 15_000 });
  expect(await out.textContent()).toBe('a {\n  position: absolute;\n  width: 1px;\n  color: #ABCDEF;\n}\n');
});

test('format-css covers enum choices and default-true selector checkbox off path', async ({ page }) => {
  await page.goto('/tools/format-css/');
  await setCss(page, 'h1,h2{margin:0;color:red;background:blue}');

  const out = page.locator('#tool-output');

  await page.selectOption('#in-sort', 'none');
  await expect(out).toContainText('margin: 0;', { timeout: 15_000 });
  expect(await out.textContent()).toBe('h1,\nh2 {\n  margin: 0;\n  color: red;\n  background: blue;\n}\n');

  await page.selectOption('#in-sort', 'alphabetical');
  await expect(out).toContainText('background: blue;', { timeout: 15_000 });
  expect(await out.textContent()).toBe('h1,\nh2 {\n  background: blue;\n  color: red;\n  margin: 0;\n}\n');

  await page.uncheck('#in-selectors_per_line');
  await expect(out).toContainText('h1, h2 {', { timeout: 15_000 });
  expect(await out.textContent()).toBe('h1, h2 {\n  background: blue;\n  color: red;\n  margin: 0;\n}\n');
});

test('format-css deep-links tab indent and max indent boundary', async ({ page }) => {
  const qs = new URLSearchParams({
    input: '.btn{color:red;&:hover{color:blue}}',
    indent: '8',
    indent_char: 'tab',
    sort: 'none',
    selectors_per_line: 'true',
    uppercase_hex: 'false',
  });
  await page.goto(`/tools/format-css/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue('.btn{color:red;&:hover{color:blue}}');
  await expect(page.locator('#in-indent')).toHaveValue('8');
  await expect(page.locator('#in-indent_char')).toHaveValue('tab');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('&:hover', { timeout: 15_000 });
  expect(await out.textContent()).toBe('.btn {\n\tcolor: red;\n\t&:hover {\n\t\tcolor: blue;\n\t}\n}\n');
});
