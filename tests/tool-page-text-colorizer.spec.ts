import { test, expect } from './fixtures';

const LOG = 'INFO service started\nWARN retrying request\nERROR database timeout';
const RULES = 'bold red: \\bERROR\\b\nyellow: \\bWARN(ING)?\\b\ngreen: \\bINFO\\b';

test('text-colorizer emits exact ANSI escapes for log levels', async ({ page }) => {
  await page.goto('/tools/text-colorizer/');
  await page.fill('#in-text', LOG);
  await page.fill('#in-rules', RULES);

  await expect(page.locator('#tool-output')).toHaveText(
    '\u001b[32mINFO\u001b[0m service started\n\u001b[33mWARN\u001b[0m retrying request\n\u001b[1;31mERROR\u001b[0m database timeout',
    { timeout: 15_000 },
  );
});

test('text-colorizer deep-link renders HTML with escaped text', async ({ page }) => {
  await page.goto('/tools/text-colorizer/?output=html&theme=light&ignore_case=true');
  await page.fill('#in-text', 'GET https://example.test 200\nERROR <timeout>');
  await page.fill('#in-rules', 'blue underline: https?://\\S+\nbold red: error|timeout');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<pre style="background:#ffffff', { timeout: 15_000 });
  await expect(out).toContainText('<span style="color:#3465a4;text-decoration:underline">https://example.test</span>');
  await expect(out).toContainText('&lt;');
  await expect(out).toContainText('<span style="color:#cc0000;font-weight:bold">ERROR</span>');
});

test('text-colorizer colors whole matching lines', async ({ page }) => {
  await page.goto('/tools/text-colorizer/?whole_line=true');
  await page.fill('#in-text', 'PASS parser tests\nFAIL wasm build\nSKIP browser smoke');
  await page.fill('#in-rules', 'bold green: ^PASS\nbold red: ^FAIL\ncyan: ^SKIP');

  await expect(page.locator('#tool-output')).toHaveText(
    '\u001b[1;32mPASS parser tests\u001b[0m\n\u001b[1;31mFAIL wasm build\u001b[0m\n\u001b[36mSKIP browser smoke\u001b[0m',
    { timeout: 15_000 },
  );
});

test('text-colorizer reports invalid regex errors', async ({ page }) => {
  await page.goto('/tools/text-colorizer/');
  await page.fill('#in-text', 'anything');
  await page.fill('#in-rules', 'red: (unclosed');
  await expect(page.locator('#tool-output')).toContainText('invalid regex', { timeout: 15_000 });
});
