import { test, expect } from './fixtures';

// Main path: default double_curly / comma / empty / divider, case-insensitive on.
// Multi-line output, so compare textContent exactly (toHaveText normalizes whitespace).
test('mail-merge page renders one document per row with the default divider', async ({ page }) => {
  await page.goto('/tools/mail-merge/');
  await page.fill('#in-template', 'Hi {{name}}, you owe ${{amount}}.');
  await page.fill('#in-csv', 'name,amount\nAlice,10\nBob,20');
  await expect
    .poll(async () => page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('Hi Alice, you owe $10.\n\n---\n\nHi Bob, you owe $20.');
});

// Enum matrix on the real page: single_curly + semicolon + newline separator.
test('mail-merge single_curly + semicolon + newline separator', async ({ page }) => {
  await page.goto('/tools/mail-merge/');
  await page.selectOption('#in-syntax', 'single_curly');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.selectOption('#in-separator', 'newline');
  await page.fill('#in-template', '{greeting} {who}');
  await page.fill('#in-csv', 'greeting;who\nHej;Ada\nHola;Bob');
  await expect
    .poll(async () => page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('Hej Ada\nHola Bob');
});

// Enum matrix: double_angle + tab delimiter + none separator.
test('mail-merge double_angle + tab delimiter', async ({ page }) => {
  await page.goto('/tools/mail-merge/');
  await page.selectOption('#in-syntax', 'double_angle');
  await page.selectOption('#in-delimiter', 'tab');
  await page.selectOption('#in-separator', 'none');
  await page.fill('#in-template', '<<a>>-<<b>>');
  await page.fill('#in-csv', 'a\tb\n1\t2');
  await expect(page.locator('#tool-output')).toHaveText('1-2', { timeout: 15000 });
});

// on_missing = keep leaves the placeholder text for an absent column.
test('mail-merge on_missing keep leaves the placeholder', async ({ page }) => {
  await page.goto('/tools/mail-merge/');
  await page.selectOption('#in-on_missing', 'keep');
  await page.selectOption('#in-separator', 'none');
  await page.fill('#in-template', '{{name}} <{{email}}>');
  await page.fill('#in-csv', 'name\nAda');
  await expect(page.locator('#tool-output')).toHaveText('Ada <{{email}}>', { timeout: 15000 });
});

// NON-default checkbox: case-insensitive OFF, so {{Name}} does not match header `name`.
test('mail-merge case-insensitive off makes an unmatched name render empty', async ({ page }) => {
  await page.goto('/tools/mail-merge/');
  await page.selectOption('#in-separator', 'none');
  await page.fill('#in-template', 'Hello {{Name}}!');
  await page.fill('#in-csv', 'name\nAda');
  // With the box still checked it renders "Hello Ada!"; unchecking must change it.
  await expect(page.locator('#tool-output')).toHaveText('Hello Ada!', { timeout: 15000 });
  await page.uncheck('#in-case_insensitive');
  await expect(page.locator('#tool-output')).toHaveText('Hello !', { timeout: 15000 });
});

// Deep-link: params prefill from the query string and auto-run.
test('mail-merge query-param deep-link', async ({ page }) => {
  const template = 'Dear {{name}},';
  const csv = 'name\nAda\nBob';
  await page.goto(
    '/tools/mail-merge/?template=' +
      encodeURIComponent(template) +
      '&csv=' +
      encodeURIComponent(csv) +
      '&separator=newline',
  );
  await expect(page.locator('#in-template')).toHaveValue(template, { timeout: 15000 });
  await expect
    .poll(async () => page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('Dear Ada,\nDear Bob,');
});
