import { test, expect } from './fixtures';

// /tools/yaml-lint/ lints YAML in-browser (pure wasm).
// input is a multiline <textarea>; preset and report_format are selects.

test('yaml-lint reports duplicate keys and style warnings', async ({ page }) => {
  await page.goto('/tools/yaml-lint/');
  await page.fill('#in-input', 'name: demo\nname: duplicate\ndebug: yes\nlist:\n  -   item\n');
  await page.selectOption('#in-preset', 'default');
  await page.fill('#in-indent_spaces', '2');
  await page.fill('#in-max_line_length', '80');
  const strict = page.locator('#in-strict_warnings');
  if (await strict.isChecked()) await strict.uncheck();
  await page.selectOption('#in-report_format', 'report');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('duplicate key', { timeout: 15000 });
  await expect(out).toContainText('truthy value');
  await expect(out).toContainText('spaces after');
  await expect(out).toContainText('[key-duplicates]');
});

test('yaml-lint can return JSON and promote warnings to errors', async ({ page }) => {
  await page.goto('/tools/yaml-lint/');
  await page.fill('#in-input', 'debug: yes\n');
  await page.selectOption('#in-preset', 'default');
  await page.fill('#in-indent_spaces', '2');
  await page.fill('#in-max_line_length', '80');
  await page.check('#in-strict_warnings');
  await page.selectOption('#in-report_format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"rule": "truthy"', { timeout: 15000 });
  await expect(out).toContainText('"errors": 1');
  await expect(out).toContainText('"warnings": 0');
});

test('yaml-lint supports deep-linked parameters', async ({ page }) => {
  const params = new URLSearchParams({
    input: '---\na: 1\n',
    preset: 'strict',
    indent_spaces: '4',
    max_line_length: '80',
    disable: 'document-start',
    strict_warnings: 'false',
    report_format: 'report',
  });
  await page.goto(`/tools/yaml-lint/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('---\na: 1\n');
  await expect(page.locator('#in-preset')).toHaveValue('strict');
  await expect(page.locator('#in-disable')).toHaveValue('document-start');
  await expect(page.locator('#tool-output')).toContainText('valid YAML', { timeout: 15000 });
});
