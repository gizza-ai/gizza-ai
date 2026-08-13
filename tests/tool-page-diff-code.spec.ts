import { test, expect } from './fixtures';

const OLD = 'fn main() {\n    println!("hi");\n}';
const NEW = 'fn main() {\n    println!("hello");\n    println!("world");\n}';

async function runWasm(
  page: any,
  left: string,
  right: string,
  view = 'side-by-side',
  granularity = 'word',
  ignoreCase = 'false',
  ignoreWhitespace = 'false',
  context = '3',
  lineNumbers = 'true',
  width = '60',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/diff-code/gizza_ai_diff_code_web.js');
    await mod.default('/tools/diff-code/gizza_ai_diff_code_web_bg.wasm');
    return mod.run(
      args.left,
      args.right,
      args.view,
      args.granularity,
      args.ignoreCase,
      args.ignoreWhitespace,
      args.context,
      args.lineNumbers,
      args.width,
    );
  }, { left, right, view, granularity, ignoreCase, ignoreWhitespace, context, lineNumbers, width });
}

test('diff-code wasm covers every advertised view exactly', async ({ page }) => {
  await page.goto('/tools/diff-code/');

  await expect(runWasm(page, OLD, NEW, 'side-by-side')).resolves.toContain('[-hi-]');
  await expect(runWasm(page, OLD, NEW, 'side-by-side')).resolves.toContain('{+hello+}');
  await expect(runWasm(page, OLD, NEW, 'unified')).resolves.toBe(
    '--- left\n' +
    '+++ right\n' +
    '@@ -1,3 +1,4 @@\n' +
    ' fn main() {\n' +
    '-    println!("hi");\n' +
    '+    println!("hello");\n' +
    '+    println!("world");\n' +
    ' }'
  );
  await expect(runWasm(page, OLD, NEW, 'word-diff')).resolves.toContain('[-hi-]{+hello+}');
  await expect(runWasm(page, OLD, NEW, 'stats')).resolves.toBe(
    'Lines:      3 left / 4 right\n' +
    'Added:      1\n' +
    'Removed:    0\n' +
    'Changed:    1\n' +
    'Unchanged:  2\n' +
    'Similarity: 50.0%\n' +
    'Word-level:  1 removed / 1 added inside changed lines'
  );
  await expect(runWasm(page, OLD, NEW, 'json')).resolves.toContain('"changed": 1');
});

test('diff-code page computes exact side-by-side output from the form', async ({ page }) => {
  await page.goto('/tools/diff-code/');
  await page.fill('#in-left', OLD);
  await page.fill('#in-right', NEW);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('[-hi-]', { timeout: 15_000 });
  await expect(out).toContainText('{+hello+}');
  await expect(out).toContainText('1 line added, 0 lines removed, 1 line changed, 2 lines unchanged');
});

test('diff-code deep-link prefills params and auto-runs stats with checkbox states', async ({ page }) => {
  const params = new URLSearchParams({
    left: 'Hello   World',
    right: 'hello world',
    view: 'stats',
    granularity: 'none',
    ignore_case: 'true',
    ignore_whitespace: 'true',
    context: '0',
    line_numbers: 'false',
    width: '20',
  });
  await page.goto(`/tools/diff-code/?${params.toString()}`);

  await expect(page.locator('#in-left')).toHaveValue('Hello   World', { timeout: 15_000 });
  await expect(page.locator('#in-view')).toHaveValue('stats');
  await expect(page.locator('#in-granularity')).toHaveValue('none');
  await expect(page.locator('#in-ignore_case')).toBeChecked();
  await expect(page.locator('#in-ignore_whitespace')).toBeChecked();
  await expect(page.locator('#in-line_numbers')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'Lines:      1 left / 1 right\n' +
    'Added:      0\n' +
    'Removed:    0\n' +
    'Changed:    0\n' +
    'Unchanged:  1\n' +
    'Similarity: 100.0%',
    { timeout: 15_000 }
  );
});

test('diff-code covers granularity, context boundary, cap boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/diff-code/');

  await expect(runWasm(page, 'var color;', 'var colour;', 'word-diff', 'char'))
    .resolves.toContain('colo{+u+}r');
  await expect(runWasm(page, OLD, NEW, 'word-diff', 'none'))
    .resolves.toContain('[-    println!("hi");-]');

  const longLeft = Array.from({ length: 101 }, (_, i) => `line ${i}`).join('\n');
  const longRight = longLeft.replace('line 50', 'line FIFTY');
  await expect(runWasm(page, longLeft, longRight, 'side-by-side', 'word', 'false', 'false', '100'))
    .resolves.toContain('line {+FIFTY+}');

  const atCap = 'x'.repeat(1_000_000);
  await expect(runWasm(page, atCap, atCap, 'stats')).resolves.toContain('Similarity: 100.0%');
  await expect(runWasm(page, `${atCap}x`, 'x', 'stats')).rejects.toThrow(/limit is 1000000/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool diff-code');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
