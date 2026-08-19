import { test, expect } from './fixtures';

const DIFF = `--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
     let x = 1;
+    let y = 2;
     println!("{x}");
 }
`;

const BOTH = `===== BEFORE: src/main.rs =====
fn main() {
    let x = 1;
    println!("{x}");
}
===== AFTER: src/main.rs =====
fn main() {
    let x = 1;
    let y = 2;
    println!("{x}");
}
`;

async function runWasm(
  page,
  diff: string,
  output = 'both',
  file = '',
  gaps = 'marker',
  lineNumbers = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/diff-extract-versions/gizza_ai_diff_extract_versions_web.js');
    await mod.default('/tools/diff-extract-versions/gizza_ai_diff_extract_versions_web_bg.wasm');
    return mod.run(args.diff, args.output, args.file, args.gaps, args.lineNumbers);
  }, { diff, output, file, gaps, lineNumbers });
}

test('diff-extract-versions wasm reconstructs both versions exactly', async ({ page }) => {
  await page.goto('/tools/diff-extract-versions/');
  await page.waitForSelector('#in-diff');

  await expect(runWasm(page, DIFF)).resolves.toBe(BOTH);
});

test('diff-extract-versions wasm covers output modes, filters, gaps, checkbox, and cap', async ({ page }) => {
  await page.goto('/tools/diff-extract-versions/');
  await page.waitForSelector('#in-diff');

  await expect(runWasm(page, DIFF, 'before')).resolves.toBe(
    'fn main() {\n    let x = 1;\n    println!("{x}");\n}\n',
  );
  await expect(runWasm(page, DIFF, 'after')).resolves.toBe(
    'fn main() {\n    let x = 1;\n    let y = 2;\n    println!("{x}");\n}\n',
  );

  const multi = `diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1 +1 @@
-alpha
+ALPHA
diff --git a/b.txt b/b.txt
--- a/b.txt
+++ b/b.txt
@@ -1 +1 @@
-bravo
+BRAVO
`;
  await expect(runWasm(page, multi, 'after', '*.txt')).resolves.toContain('===== b.txt =====\nBRAVO\n');
  await expect(runWasm(page, multi, 'after', 'b.txt')).resolves.toBe('BRAVO\n');

  const gap = `--- a/list.txt
+++ b/list.txt
@@ -9,2 +9,2 @@
-nine
+NINE
 ten
`;
  await expect(runWasm(page, gap, 'before', '', 'marker', 'true')).resolves.toBe(
    '   | [... 8 lines not in the diff (lines 1-8) ...]\n 9 | nine\n10 | ten\n',
  );
  await expect(runWasm(page, gap, 'before', '', 'omit', 'true')).resolves.toBe(' 9 | nine\n10 | ten\n');
  await expect(runWasm(page, gap, 'before', '', 'error')).rejects.toThrow(/incomplete/);

  const json = JSON.parse(await runWasm(page, DIFF, 'json'));
  expect(json.files[0]).toMatchObject({
    before_path: 'src/main.rs',
    after_path: 'src/main.rs',
    status: 'modified',
    hunks: 1,
    added: 1,
    removed: 0,
    complete: true,
  });

  const atCap = DIFF + '#'.repeat(1_000_000 - DIFF.length);
  await expect(runWasm(page, atCap, 'both')).resolves.toContain('BEFORE: src/main.rs');
  await expect(runWasm(page, `${atCap}x`, 'both')).rejects.toThrow(/limit is 1000000 bytes/);
});

test('diff-extract-versions page renders output from form controls', async ({ page }) => {
  await page.goto('/tools/diff-extract-versions/');
  await page.fill('#in-diff', DIFF);

  await expect(page.locator('#tool-output')).toHaveText(BOTH, { timeout: 15_000 });

  await page.selectOption('#in-output', 'after');
  await expect(page.locator('#tool-output')).toHaveText(
    'fn main() { let x = 1; let y = 2; println!("{x}"); }',
    { timeout: 15_000 },
  );
});

test('diff-extract-versions deep-link prefills controls and renders numbered before text', async ({ page }) => {
  const params = new URLSearchParams({
    diff: DIFF,
    output: 'before',
    file: 'main.rs',
    gaps: 'marker',
    line_numbers: 'true',
  });

  await page.goto(`/tools/diff-extract-versions/?${params.toString()}`);
  await expect(page.locator('#in-diff')).toHaveValue(DIFF, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('before');
  await expect(page.locator('#in-file')).toHaveValue('main.rs');
  await expect(page.locator('#in-gaps')).toHaveValue('marker');
  await expect(page.locator('#in-line_numbers')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    '1 | fn main() { 2 | let x = 1; 3 | println!("{x}"); 4 | }',
    { timeout: 15_000 },
  );
});
