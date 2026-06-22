import { test, expect } from './fixtures';

// /tools/markdown-table-format/ aligns + pretty-prints GFM tables in-browser (pure wasm).
// markdown is a multiline <textarea>; align is a <select> (keep | left | center | right).

test('markdown-table-format aligns a ragged table (keep)', async ({ page }) => {
  await page.goto('/tools/markdown-table-format/');
  await page.fill('#in-markdown', '|Name|Role|\n|---|---|\n|Ada|Engineer|\n|Bo|Designer|');
  await page.selectOption('#in-align', 'keep');
  const out = page.locator('#tool-output');
  // Cells padded to even widths and separators lined up.
  await expect(out).toContainText('| Name | Role     |', { timeout: 15000 });
  await expect(out).toContainText('| Ada  | Engineer |');
  await expect(out).toContainText('| Bo   | Designer |');
});

test('markdown-table-format preserves alignment markers', async ({ page }) => {
  await page.goto('/tools/markdown-table-format/');
  await page.fill('#in-markdown', '| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |');
  await page.selectOption('#in-align', 'keep');
  const out = page.locator('#tool-output');
  const text = (await out.textContent({ timeout: 15000 }))!;
  expect(text).toContain(':--');
  expect(text).toContain(':-:');
  expect(text).toContain('--:');
});

test('markdown-table-format forces right alignment', async ({ page }) => {
  await page.goto('/tools/markdown-table-format/');
  await page.fill('#in-markdown', '|a|bb|\n|---|---|\n|1|2|');
  await page.selectOption('#in-align', 'right');
  const out = page.locator('#tool-output');
  const text = (await out.textContent({ timeout: 15000 }))!;
  // Delimiter row carries the right marker for every column.
  const lines = text.split('\n');
  expect(lines[1]).toContain('--:');
  expect(lines[1]).not.toContain(':--');
});

test('markdown-table-format compact style trims padding', async ({ page }) => {
  await page.goto('/tools/markdown-table-format/');
  await page.fill('#in-markdown', '|Name|Role|\n|---|---|\n|Ada|Engineer|\n|Bo|Designer|');
  await page.selectOption('#in-align', 'keep');
  await page.selectOption('#in-style', 'compact');
  const out = page.locator('#tool-output');
  // Compact: no width padding, so the header keeps single-space cells.
  await expect(out).toContainText('| Name | Role |', { timeout: 15000 });
  await expect(out).toContainText('| Ada | Engineer |');
});

test('markdown-table-format leaves tables in code fences alone', async ({ page }) => {
  await page.goto('/tools/markdown-table-format/');
  await page.fill('#in-markdown', '```\n|a|b|\n|---|---|\n|1|2|\n```\n\n| x | y |\n|---|---|\n| 9 | 8 |');
  await page.selectOption('#in-style', 'pretty');
  const out = page.locator('#tool-output');
  const text = (await out.textContent({ timeout: 15000 }))!;
  // Fenced table kept verbatim; the table outside the fence is reformatted.
  expect(text).toContain('|a|b|');
  expect(text).toContain('| x   | y   |');
});
