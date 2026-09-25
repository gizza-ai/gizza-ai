import { test, expect } from './fixtures';

async function setInput(page: any, value: string) {
  await page.$eval(
    '#in-input',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('convert-quotes page converts single quoted code to double quoted code', async ({ page }) => {
  await page.goto('/tools/convert-quotes/');
  await setInput(page, "print('hello')\ngreeting = 'it\\'s a test'\ntitle = 'He said \"hi\"'");

  const out = page.locator('#tool-output');
  await expect(out).toContainText('print("hello")', { timeout: 15000 });
  await expect(out).toContainText('greeting = "it\'s a test"');
  await expect(out).toContainText('title = "He said \\"hi\\""');
});

test('convert-quotes page supports double to single with SQL doubled escaping', async ({ page }) => {
  await page.goto('/tools/convert-quotes/');
  await setInput(page, 'INSERT INTO t VALUES ("it\'s here", "O\'Hara");');
  await page.selectOption('#in-direction', 'double-to-single');
  await page.selectOption('#in-escape_style', 'doubled');

  const out = page.locator('#tool-output');
  await expect(out).toContainText("INSERT INTO t VALUES ('it''s here', 'O''Hara');", { timeout: 15000 });
});

test('convert-quotes query params prefill every control and return a JSON report', async ({ page }) => {
  const input = "'a' \"b\" “c” ‘d’ and 'oops";
  await page.goto(
    '/tools/convert-quotes/?input=' +
      encodeURIComponent(input) +
      '&direction=auto-to-double&escape_style=backslash&preserve_apostrophes=false&on_unbalanced=keep&include_report=true',
  );

  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-direction')).toHaveValue('auto-to-double');
  await expect(page.locator('#in-escape_style')).toHaveValue('backslash');
  await expect(page.locator('#in-preserve_apostrophes')).not.toBeChecked();
  await expect(page.locator('#in-on_unbalanced')).toHaveValue('keep');
  await expect(page.locator('#in-include_report')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"result": "\\"a\\" \\"b\\" \\"c\\" \\"d\\" and \'oops"', { timeout: 15000 });
  await expect(out).toContainText('"converted": 4');
  await expect(out).toContainText('"unbalanced": 1');
});
