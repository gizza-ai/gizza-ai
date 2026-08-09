import { test, expect } from './fixtures';

const SRC = `fn main() {
    let x = 1;
    println!("{x}");
}
`;

const PATCH = `--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
     let x = 1;
+    let y = 2;
     println!("{x}");
 }
`;

const PATCHED = `fn main() {
    let x = 1;
    let y = 2;
    println!("{x}");
}
`;

// Two extra leading lines (offset) AND a drifted trailing context line (fuzz).
const DRIFTED = `// header
// header2
fn main() {
    let x = 1;
    println!("{x}");
} // trailing comment
`;

// The second hunk's context does not exist in the source — a real conflict.
const LIST_SRC = `alpha
bravo
charlie
delta
echo
foxtrot
golf
hotel
`;

const LIST_PATCH = `--- a/list.txt
+++ b/list.txt
@@ -1,2 +1,3 @@
 alpha
+ADDED
 bravo
@@ -6,2 +7,2 @@
-NOPE
+YES
 hotel
`;

async function runWasm(
  page,
  source: string,
  patch: string,
  output = 'patched',
  reverse = 'false',
  fuzz = '2',
  ignoreWhitespace = 'false',
  onConflict = 'fail',
  file = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/apply-patch/gizza_ai_apply_patch_web.js');
    await mod.default('/tools/apply-patch/gizza_ai_apply_patch_web_bg.wasm');
    return mod.run(
      args.source,
      args.patch,
      args.output,
      args.reverse,
      args.fuzz,
      args.ignoreWhitespace,
      args.onConflict,
      args.file,
    );
  }, { source, patch, output, reverse, fuzz, ignoreWhitespace, onConflict, file });
}

test('apply-patch wasm returns the exact patched file', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.waitForSelector('#in-source');

  await expect(runWasm(page, SRC, PATCH)).resolves.toBe(PATCHED);
});

test('apply-patch wasm covers every output mode, reverse, and on_conflict choice', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.waitForSelector('#in-source');

  // output=report — a dry run that never throws on a conflict.
  const report = await runWasm(page, LIST_SRC, LIST_PATCH, 'report', 'false', '0');
  expect(report).toContain('1 of 2 hunks applied · list.txt');
  expect(report).toContain('[1] @@ -1,2 +1,3 @@  applied at line 1');
  expect(report).toContain('FAILED — expected `NOPE` at line 6, found `foxtrot`');

  // output=json — machine-readable statuses plus the patched text.
  const json = JSON.parse(await runWasm(page, SRC, PATCH, 'json'));
  expect(json.file).toBe('src/main.rs');
  expect(json.clean).toBe(true);
  expect(json.hunks_applied).toBe(1);
  expect(json.hunks[0]).toMatchObject({ status: 'applied', applied_at_line: 1, offset: 0 });
  expect(json.patched).toBe(PATCHED);

  // output=rejects — the failed hunk re-emitted as a valid patch.
  await expect(runWasm(page, LIST_SRC, LIST_PATCH, 'rejects', 'false', '0', 'false', 'skip'))
    .resolves.toBe('--- a/list.txt\n+++ b/list.txt\n@@ -6,2 +7,2 @@\n-NOPE\n+YES\n hotel\n');
  await expect(runWasm(page, SRC, PATCH, 'rejects'))
    .resolves.toBe('No rejected hunks — every hunk applied cleanly.');

  // on_conflict=fail (default) refuses, naming the expected and found lines.
  await expect(runWasm(page, LIST_SRC, LIST_PATCH, 'patched', 'false', '0'))
    .rejects.toThrow(/conflict: hunk \[2\].*expected `NOPE` at line 6, found `foxtrot`/s);
  // on_conflict=skip applies the hunk that matches and leaves the rest.
  await expect(runWasm(page, LIST_SRC, LIST_PATCH, 'patched', 'false', '0', 'false', 'skip'))
    .resolves.toBe('alpha\nADDED\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n');

  // NON-default checkbox state: reverse=true unapplies the patch.
  await expect(runWasm(page, PATCHED, PATCH, 'patched', 'true')).resolves.toBe(SRC);
});

test('apply-patch wasm honours the fuzz boundary, ignore_whitespace, and the size cap', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.waitForSelector('#in-source');

  // fuzz 0 (slider minimum) requires every context line to match.
  await expect(runWasm(page, DRIFTED, PATCH, 'patched', 'false', '0'))
    .rejects.toThrow(/conflict: hunk \[1\]/);
  // fuzz 1 drops one context line per end — applies with an offset.
  await expect(runWasm(page, DRIFTED, PATCH, 'patched', 'false', '1')).resolves.toBe(
    '// header\n// header2\nfn main() {\n    let x = 1;\n    let y = 2;\n    println!("{x}");\n} // trailing comment\n',
  );
  const fuzzReport = await runWasm(page, DRIFTED, PATCH, 'report', 'false', '1');
  expect(fuzzReport).toContain('applied at line 3 (offset +2, fuzz 1)');
  // fuzz 3 is the slider maximum; one over is rejected by name.
  await expect(runWasm(page, DRIFTED, PATCH, 'patched', 'false', '3')).resolves.toContain('let y = 2;');
  await expect(runWasm(page, DRIFTED, PATCH, 'patched', 'false', '4')).rejects.toThrow(/fuzz must be 0-3/);

  // NON-default checkbox state: ignore_whitespace matches a re-indented file and
  // keeps the SOURCE's own tab in the output.
  const tabbed = 'fn main() {\n\tlet x = 1;\n    println!("{x}");\n}\n';
  await expect(runWasm(page, tabbed, PATCH, 'patched', 'false', '0')).rejects.toThrow(/conflict/);
  await expect(runWasm(page, tabbed, PATCH, 'patched', 'false', '0', 'true')).resolves.toBe(
    'fn main() {\n\tlet x = 1;\n    let y = 2;\n    println!("{x}");\n}\n',
  );

  // A multi-file patch needs the file filter; the filter picks one path.
  const multi =
    '--- a/one.txt\n+++ b/one.txt\n@@ -1 +1 @@\n-a\n+A\n--- a/two.txt\n+++ b/two.txt\n@@ -1 +1 @@\n-b\n+B\n';
  await expect(runWasm(page, 'a\n', multi)).rejects.toThrow(/touches 2 files \(one\.txt, two\.txt\)/);
  await expect(runWasm(page, 'b\n', multi, 'patched', 'false', '0', 'false', 'fail', 'two.txt'))
    .resolves.toBe('B\n');

  // Exact 1 MB cap boundary, at and one over.
  const head = 'one\ntwo\n';
  const atCap = head + 'x\n'.repeat((1_000_000 - head.length) / 2);
  expect(atCap.length).toBe(1_000_000);
  const capPatch = '@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n';
  await expect(runWasm(page, atCap, capPatch, 'patched', 'false', '0')).resolves.toContain('one\nTWO\n');
  await expect(runWasm(page, `${atCap}x`, capPatch, 'patched', 'false', '0'))
    .rejects.toThrow(/source text is too large/);
});

test('apply-patch page applies a patch from the form controls', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.fill('#in-source', SRC);
  await page.fill('#in-patch', PATCH);

  await expect(page.locator('#tool-output')).toHaveText(PATCHED, { timeout: 15_000 });

  // The reverse checkbox is a real control: tick it and paste the patched file
  // back to recover the original.
  await page.fill('#in-source', PATCHED);
  await page.check('#in-reverse');
  await expect(page.locator('#tool-output')).toHaveText(SRC, { timeout: 15_000 });
});

test('apply-patch page surfaces a conflict and the rejected hunks', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.fill('#in-source', LIST_SRC);
  await page.fill('#in-patch', LIST_PATCH);
  await page.fill('#in-fuzz', '0');

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 15_000 });
  await expect(out).toContainText('expected `NOPE` at line 6, found `foxtrot`');

  await page.selectOption('#in-on_conflict', 'skip');
  await expect(out).toHaveText('alpha ADDED bravo charlie delta echo foxtrot golf hotel', {
    timeout: 15_000,
  });

  await page.selectOption('#in-output', 'rejects');
  await expect(out).toContainText('@@ -6,2 +7,2 @@');
  await expect(out).toContainText('-NOPE');
});

test('apply-patch page slider mirrors the fuzz value onto the number field', async ({ page }) => {
  await page.goto('/tools/apply-patch/');
  await page.fill('#in-source', DRIFTED);
  await page.fill('#in-patch', PATCH);
  await page.fill('#in-fuzz', '0');
  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 15_000 });

  await page.locator('#in-fuzz-slider').evaluate((el: HTMLInputElement) => {
    el.value = '1';
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await expect(page.locator('#in-fuzz')).toHaveValue('1');
  await expect(page.locator('#tool-output')).toContainText('let y = 2;', { timeout: 15_000 });
});

test('apply-patch page pre-fills and computes from ?param= deep links', async ({ page }) => {
  const q = new URLSearchParams({
    source: LIST_SRC,
    patch: LIST_PATCH,
    output: 'report',
    fuzz: '0',
  });
  await page.goto(`/tools/apply-patch/?${q.toString()}`);

  await expect(page.locator('#in-source')).toHaveValue(LIST_SRC);
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-fuzz')).toHaveValue('0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 of 2 hunks applied · list.txt', { timeout: 15_000 });
  await expect(out).toContainText('FAILED — expected `NOPE` at line 6, found `foxtrot`');
});
