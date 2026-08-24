import { test, expect } from './fixtures';

// /tools/config-file-validator/ validates config syntax in-browser.
test('config-file-validator page reports a YAML indentation error', async ({ page }) => {
  await page.goto('/tools/config-file-validator/');
  await page.fill('#in-input', 'server:\n  host: localhost\n   port: 8080');
  await page.selectOption('#in-format', 'yaml');
  await page.fill('#in-context_lines', '1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — YAML', { timeout: 15000 });
  await expect(out).toContainText('line');
  await expect(out).toContainText('port: 8080');
});

test('config-file-validator page deep-link emits JSON diagnostics', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('{\n  "a": 1,\n}') +
    '&format=json&strict=false&report_format=json&context_lines=0';
  await page.goto('/tools/config-file-validator/' + qs);

  await expect(page.locator('#in-report_format')).toHaveValue('json', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15000 });
  await expect(out).toContainText('"format": "json"');
  await expect(out).toContainText('"line": 3');
});

test('config-file-validator page validates TOML and strict warnings', async ({ page }) => {
  await page.goto('/tools/config-file-validator/');
  await page.fill('#in-input', '[server]\nhost = "localhost"\nport = 8080');
  await page.selectOption('#in-format', 'toml');
  await page.check('#in-strict');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('VALID — TOML', { timeout: 15000 });
  await expect(out).toContainText('No syntax errors found');
});
