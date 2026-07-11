import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('curl-command-parser page parses a POST JSON command', async ({ page }) => {
  await page.goto('/tools/curl-command-parser/');
  await page.selectOption('#in-mode', 'parse');
  await page.fill(
    '#in-command',
    "curl -X POST 'https://api.example.com/v1/items?page=2' -H 'Content-Type: application/json' -H 'X-Trace: abc' --data-raw '{\"name\":\"gizza\"}'",
  );
  await expect(page.locator('#tool-output')).toContainText('Method:         POST', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('URL:            https://api.example.com/v1/items?page=2');
  expect(text).toContain('Content-Type:   application/json');
  expect(text).toContain('Headers (2):');
  expect(text).toContain('Content-Type: application/json');
  expect(text).toContain('X-Trace: abc');
  expect(text).toContain('Query params (1):');
  expect(text).toContain('page = 2');
  expect(text).toContain('Body (data-raw):');
  expect(text).toContain('{"name":"gizza"}');
});

test('curl-command-parser page rebuilds a messy command exactly', async ({ page }) => {
  await page.goto('/tools/curl-command-parser/');
  await page.selectOption('#in-mode', 'rebuild');
  await page.fill(
    '#in-command',
    "curl   -sSL   -XPOST https://api.example.com/upload   -F file=@photo.png   -F title=Holiday   --compressed",
  );
  await expect(page.locator('#tool-output')).toContainText('curl -X POST https://api.example.com/upload', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      'curl -X POST https://api.example.com/upload \\',
      '  -F file=@photo.png \\',
      '  -F title=Holiday \\',
      '  -L \\',
      '  --compressed',
    ].join('\n'),
  );
});

test('curl-command-parser deep-link pre-fills and auto-runs parse mode', async ({ page }) => {
  const command = encodeURIComponent("curl -G 'https://api.example.com/search' -d q=rust -d limit=10 -H 'Accept: application/json'");
  await page.goto(`/tools/curl-command-parser/?mode=parse&command=${command}`);
  await expect(page.locator('#in-command')).toHaveValue("curl -G 'https://api.example.com/search' -d q=rust -d limit=10 -H 'Accept: application/json'", { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Method:         GET', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('Flags:          get (-G)');
  expect(text).toContain('Query params (2):');
  expect(text).toContain('q = rust');
  expect(text).toContain('limit = 10');
});
