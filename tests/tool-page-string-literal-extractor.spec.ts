import { test, expect } from './fixtures';

test('string-literal-extractor lists JS literals with line numbers and skips comments', async ({ page }) => {
  await page.goto('/tools/string-literal-extractor/');
  await page.fill('#in-code', 'const greeting = "Hello, world";\n// "this comment is skipped"\nconst name = \'gizza\';\nconst tpl = `id-${x}`;');
  await page.selectOption('#in-language', 'javascript');
  await page.selectOption('#in-quotes', 'all');
  await page.selectOption('#in-format', 'list');
  await page.check('#in-line_numbers');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Hello, world  [L1]', { timeout: 15000 });
  await expect(out).toContainText('gizza  [L3]');
  await expect(out).toContainText('id-${x}  [L4]');
  await expect(out).not.toContainText('this comment is skipped');
});

test('string-literal-extractor emits JSON with positions and quote style', async ({ page }) => {
  await page.goto('/tools/string-literal-extractor/');
  await page.fill('#in-code', 'def greet():\n    title = "Report"\n    note = \'draft\'\n    return title  # "not this"');
  await page.selectOption('#in-language', 'python');
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"value": "Report"', { timeout: 15000 });
  await expect(out).toContainText('"quote": "double"');
  await expect(out).toContainText('"value": "draft"');
  await expect(out).toContainText('"quote": "single"');
  await expect(out).not.toContainText('not this');
});

test('string-literal-extractor supports deep-linked unique double-quoted filter', async ({ page }) => {
  const params = new URLSearchParams({
    code: 'fetch("https://a.example")\nfetch("https://a.example")\nfetch("https://b.example")',
    language: 'javascript',
    quotes: 'double',
    format: 'list',
    unique: 'true',
  });
  await page.goto(`/tools/string-literal-extractor/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('https://a.example', { timeout: 15000 });
  await expect(out).toContainText('https://b.example');
  // dedupe keeps a single a.example line
  const text = await out.innerText();
  const count = (text.match(/https:\/\/a\.example/g) || []).length;
  expect(count).toBe(1);
});
