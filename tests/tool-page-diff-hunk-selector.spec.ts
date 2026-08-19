import { test, expect } from './fixtures';

const DIFF = `diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("hello");
 }
@@ -10,2 +11,2 @@
-old_call();
+new_call();
`;

const LIST = `1 file · 2 hunks · +2 −1

src/main.rs · 2 hunks · +2 −1
 [1] @@ -1,2 +1,3 @@    +1 −0
 [2] @@ -10,2 +11,2 @@  +1 −1`;

const PATCH_2 = `diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,2 +10,2 @@
-old_call();
+new_call();

`;

async function runWasm(
  page,
  diff: string,
  output = 'list',
  hunks = 'all',
  invert = 'false',
  files = '',
  lines = '',
  renumber = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/diff-hunk-selector/gizza_ai_diff_hunk_selector_web.js');
    await mod.default('/tools/diff-hunk-selector/gizza_ai_diff_hunk_selector_web_bg.wasm');
    return mod.run(args.diff, args.output, args.hunks, args.invert, args.files, args.lines, args.renumber);
  }, { diff, output, hunks, invert, files, lines, renumber });
}

test('diff-hunk-selector wasm lists hunks exactly', async ({ page }) => {
  await page.goto('/tools/diff-hunk-selector/');
  await page.waitForSelector('#in-diff');

  await expect(runWasm(page, DIFF)).resolves.toBe(LIST);
});

test('diff-hunk-selector wasm covers output modes, selections, filters, checkbox states, and cap', async ({ page }) => {
  await page.goto('/tools/diff-hunk-selector/');
  await page.waitForSelector('#in-diff');

  await expect(runWasm(page, DIFF, 'patch', '2')).resolves.toBe(PATCH_2);
  await expect(runWasm(page, DIFF, 'patch', '1', 'true')).resolves.toBe(PATCH_2);
  await expect(runWasm(page, DIFF, 'patch', 'all', 'false', '*.rs', '10-12', 'false')).resolves.toContain('@@ -10,2 +11,2 @@');
  await expect(runWasm(page, DIFF, 'split', '1-2')).resolves.toContain('==== patch 2 of 2 · hunk [2] · src/main.rs ====');
  const json = await runWasm(page, DIFF, 'json', '-1');
  expect(JSON.parse(json).selection.selected).toEqual([1]);

  const base = '--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-a\n+b\n';
  const atCap = base + '#'.repeat(1_000_000 - base.length);
  await expect(runWasm(page, atCap, 'list')).resolves.toContain('f.txt · 1 hunk');
  const overCap = `${atCap}x`;
  await expect(runWasm(page, overCap, 'list')).rejects.toThrow(/too large/);
});

test('diff-hunk-selector page renders patch output from form controls', async ({ page }) => {
  await page.goto('/tools/diff-hunk-selector/');
  await page.fill('#in-diff', DIFF);
  await page.selectOption('#in-output', 'patch');
  await page.fill('#in-hunks', '2');

  await expect(page.locator('#tool-output')).toHaveText(PATCH_2, { timeout: 15_000 });
});

test('diff-hunk-selector deep-link prefills controls and renders JSON', async ({ page }) => {
  const params = new URLSearchParams({
    diff: DIFF,
    output: 'json',
    hunks: '1',
    invert: 'true',
    files: '*.rs',
    lines: '',
    renumber: 'false',
  });

  await page.goto(`/tools/diff-hunk-selector/?${params.toString()}`);
  await expect(page.locator('#in-diff')).toHaveValue(DIFF, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-hunks')).toHaveValue('1');
  await expect(page.locator('#in-invert')).toBeChecked();
  await expect(page.locator('#in-files')).toHaveValue('*.rs');
  await expect(page.locator('#in-renumber')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toContainText('"selected": [', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('2');
});
