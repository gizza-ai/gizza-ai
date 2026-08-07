import { test, expect } from './fixtures';

const JS_INPUT = '// File header\nconst url = "https://example.com//path"; // Real trailing comment\n/* Multi-line\n   block note */\nconsole.log(url);';

async function outputText(page: any): Promise<string> {
  const output = page.locator('#tool-output');
  await expect(output).toContainText('File header', { timeout: 15_000 });
  return (await output.textContent()) ?? '';
}

test('code-comment-extractor page lists JavaScript comments with line numbers', async ({ page }) => {
  await page.goto('/tools/code-comment-extractor/');
  await page.waitForSelector('#in-code');
  await page.fill('#in-code', JS_INPUT);
  await page.selectOption('#in-language', 'javascript');
  await page.selectOption('#in-output', 'comments');
  await page.selectOption('#in-kind', 'all');
  await page.check('#in-strip_markers');
  await page.check('#in-line_numbers');
  await page.fill('#in-min_length', '0');
  const text = await outputText(page);
  expect(text).toContain('[L1] File header');
  expect(text).toContain('[L2] Real trailing comment');
  expect(text).toContain('[L3] Multi-line\nblock note');
  expect(text).not.toContain('https://example.com//path');
});

test('code-comment-extractor deep link emits JSON doc comments', async ({ page }) => {
  const params = new URLSearchParams({
    code: '/** Parse a value. */\n/// Returns null for blank input.\nfn parse() {\n  let s = r#"// not a comment"#;\n}',
    language: 'rust',
    output: 'json',
    kind: 'doc',
    strip_markers: 'true',
    line_numbers: 'false',
    min_length: '0',
    docstrings: 'true',
  });
  await page.goto(`/tools/code-comment-extractor/?${params.toString()}`);
  await page.waitForSelector('#in-code');
  await expect(page.locator('#in-language')).toHaveValue('rust', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('Parse a value.', { timeout: 15_000 });
  const text = (await output.textContent()) ?? '';
  const comments = JSON.parse(text);
  expect(comments).toEqual([
    { line: 1, column: 1, end_line: 1, kind: 'doc', text: 'Parse a value.' },
    { line: 2, column: 1, end_line: 2, kind: 'doc', text: 'Returns null for blank input.' },
  ]);
});
