import { test, expect } from './fixtures';

const BASIC_INPUT = 'if x then\nprint(1)\nend';
const BASIC_OUTPUT = 'if x then\n  print(1)\nend';

test('lua-formatter page re-indents an if block and normalizes quote style', async ({ page }) => {
  await page.goto('/tools/lua-formatter/');
  await page.fill('#in-input', "local s = 'hi'\nif x then\nprint(s)\nend");
  await page.fill('#in-indent', '4');
  await page.selectOption('#in-indent_char', 'space');
  await page.selectOption('#in-quote_style', 'double');
  await expect(page.locator('#tool-output')).toHaveText(
    'local s = "hi"\nif x then\n    print(s)\nend',
    { timeout: 15_000 },
  );
});

test('lua-formatter deep-link prefills params and renders tabs', async ({ page }) => {
  const params = new URLSearchParams({
    input: BASIC_INPUT,
    indent: '2',
    indent_char: 'tab',
    quote_style: 'preserve',
  });
  await page.goto(`/tools/lua-formatter/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(BASIC_INPUT, { timeout: 15_000 });
  await expect(page.locator('#in-indent')).toHaveValue('2');
  await expect(page.locator('#in-indent_char')).toHaveValue('tab');
  await expect(page.locator('#in-quote_style')).toHaveValue('preserve');
  await expect(page.locator('#tool-output')).toHaveText('if x then\n\tprint(1)\nend', {
    timeout: 15_000,
  });
});

test('lua-formatter supports exact boundary indent and single quotes', async ({ page }) => {
  await page.goto('/tools/lua-formatter/');
  await page.fill('#in-input', 'do\nx = "hi"\nend');
  await page.fill('#in-indent', '8');
  await page.selectOption('#in-indent_char', 'space');
  await page.selectOption('#in-quote_style', 'single');
  await expect(page.locator('#tool-output')).toHaveText("do\n        x = 'hi'\nend", {
    timeout: 15_000,
  });
});
