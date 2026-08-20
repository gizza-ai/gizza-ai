import { test, expect } from './fixtures';

const SIMPLE = `diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
 }
`;

const SIMPLE_REVERSED = `diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,3 @@
 fn main() {
-    println!("hello");
 }
`;

const MULTI = `--- a/one.txt
+++ b/one.txt
@@ -1 +1 @@
-a
+b
--- a/dir/two.txt
+++ b/dir/two.txt
@@ -1 +1 @@
-c
+d
`;

async function runWasm(
  page,
  diff: string,
  output = 'patch',
  file = '',
  indexLines = 'swap',
  swapPaths = 'true',
  onBinary = 'fail',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/unified-diff-reverse/gizza_ai_unified_diff_reverse_web.js');
    await mod.default('/tools/unified-diff-reverse/gizza_ai_unified_diff_reverse_web_bg.wasm');
    return mod.run(args.diff, args.output, args.file, args.indexLines, args.swapPaths, args.onBinary);
  }, { diff, output, file, indexLines, swapPaths, onBinary });
}

test('unified-diff-reverse wasm returns the exact reverse patch', async ({ page }) => {
  await page.goto('/tools/unified-diff-reverse/');
  await page.waitForSelector('#in-diff');

  await expect(runWasm(page, SIMPLE)).resolves.toBe(SIMPLE_REVERSED);
});

test('unified-diff-reverse wasm covers outputs, enum choices, checkbox state, file filter, binary modes, and cap', async ({ page }) => {
  await page.goto('/tools/unified-diff-reverse/');
  await page.waitForSelector('#in-diff');

  const summary = await runWasm(page, SIMPLE, 'summary');
  expect(summary).toContain('Reverse patch · 1 file · 1 hunk · +0 −1');
  expect(summary).toContain('src/main.rs · 1 hunk · +0 −1');

  const json = JSON.parse(await runWasm(page, SIMPLE, 'json'));
  expect(json.files).toBe(1);
  expect(json.patch).toBe(SIMPLE_REVERSED);

  const indexed = `diff --git a/app.js b/app.js
index 1111111..2222222 100644
--- a/app.js
+++ b/app.js
@@ -1 +1 @@
-oldCall();
+newCall();
`;
  await expect(runWasm(page, indexed, 'patch', '', 'swap')).resolves.toContain('index 2222222..1111111 100644');
  await expect(runWasm(page, indexed, 'patch', '', 'keep')).resolves.toContain('index 1111111..2222222 100644');
  await expect(runWasm(page, indexed, 'patch', '', 'drop')).resolves.not.toContain('index ');
  await expect(runWasm(page, indexed, 'patch', '', 'swap', 'false')).resolves.toContain('diff --git a/app.js b/app.js');

  await expect(runWasm(page, MULTI, 'patch', 'two.txt')).resolves.toBe('--- a/dir/two.txt\n+++ b/dir/two.txt\n@@ -1 +1 @@\n-d\n+c\n');

  const binary = `diff --git a/img.png b/img.png
index 111..222 100644
GIT binary patch
literal 4
zcmZ
`;
  await expect(runWasm(page, binary, 'patch', '', 'swap', 'true', 'fail')).rejects.toThrow(/binary patch for img\.png/);
  await expect(runWasm(page, `${SIMPLE}${binary}`, 'patch', '', 'swap', 'true', 'skip')).resolves.toBe(SIMPLE_REVERSED);
  await expect(runWasm(page, binary, 'patch', '', 'swap', 'true', 'keep')).resolves.toContain('GIT binary patch');

  const base = '--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-a\n+b\n';
  const atCap = base + '#'.repeat(1_000_000 - base.length);
  await expect(runWasm(page, atCap)).resolves.toContain('--- a/f.txt');
  await expect(runWasm(page, `${atCap}x`)).rejects.toThrow(/over the 1000000 byte/);
});

test('unified-diff-reverse page renders from form controls', async ({ page }) => {
  await page.goto('/tools/unified-diff-reverse/');
  await page.fill('#in-diff', SIMPLE);

  await expect(page.locator('#tool-output')).toHaveText(SIMPLE_REVERSED, { timeout: 15_000 });

  await page.selectOption('#in-output', 'summary');
  await expect(page.locator('#tool-output')).toContainText('Reverse patch · 1 file · 1 hunk', { timeout: 15_000 });
});

test('unified-diff-reverse deep-link prefills controls and renders JSON', async ({ page }) => {
  const params = new URLSearchParams({
    diff: SIMPLE,
    output: 'json',
    index_lines: 'drop',
    swap_paths: 'false',
    on_binary: 'skip',
  });

  await page.goto(`/tools/unified-diff-reverse/?${params.toString()}`);
  await expect(page.locator('#in-diff')).toHaveValue(SIMPLE, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-index_lines')).toHaveValue('drop');
  await expect(page.locator('#in-swap_paths')).not.toBeChecked();
  await expect(page.locator('#in-on_binary')).toHaveValue('skip');
  await expect(page.locator('#tool-output')).toContainText('"files": 1', { timeout: 15_000 });
});
