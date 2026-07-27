import { test, expect } from './fixtures';

const LOG = '2026 INFO start\n2026 DEBUG state={"user":"gizza","ok":true}\n2026 INFO ids [1, 2, 3] queued';
const BLOCKS_OUTPUT = `// block 1 (line 2)
{
  "user": "gizza",
  "ok": true
}

// block 2 (line 3)
[
  1,
  2,
  3
]`;

test('json-from-logs page extracts embedded JSON blocks exactly', async ({ page }) => {
  await page.goto('/tools/json-from-logs/');
  await page.fill('#in-text', LOG);
  const output = page.locator('#tool-output');
  await expect(output).toHaveText(BLOCKS_OUTPUT, { timeout: 15_000 });
});

test('json-from-logs deep link prefills array output and indent', async ({ page }) => {
  const text = encodeURIComponent('req {"path":"/pets"} resp {"status":200}');
  await page.goto(`/tools/json-from-logs/?text=${text}&output=array&indent=0`);
  await expect(page.locator('#in-output')).toHaveValue('array', { timeout: 15_000 });
  await expect(page.locator('#in-indent')).toHaveValue('0');
  const output = page.locator('#tool-output');
  await expect(output).toHaveText('[{"path":"/pets"},{"status":200}]', { timeout: 15_000 });
});
