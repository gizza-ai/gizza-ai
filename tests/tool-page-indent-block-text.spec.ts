import { test, expect } from './fixtures';

async function runWasm(page, text: string, mode = 'indent', style = 'spaces', count = '4', prefix = '', lines = 'all', skipBlankLines = 'true') {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/indent-block-text/gizza_ai_indent_block_text_web.js');
    await mod.default('/tools/indent-block-text/gizza_ai_indent_block_text_web_bg.wasm');
    return mod.run(args.text, args.mode, args.style, args.count, args.prefix, args.lines, args.skipBlankLines);
  }, { text, mode, style, count, prefix, lines, skipBlankLines });
}

test('indent-block-text wasm indents every non-blank line with spaces', async ({ page }) => {
  await page.goto('/tools/indent-block-text/');
  await page.waitForSelector('#in-text');

  const out = await runWasm(page, 'a\nb\n');
  expect(out).toBe('    a\n    b\n');
});

test('indent-block-text wasm covers custom prefix, outdent, dedent, hanging, and blank checkbox', async ({ page }) => {
  await page.goto('/tools/indent-block-text/');
  await page.waitForSelector('#in-text');

  await expect(runWasm(page, 'alpha\nbeta', 'indent', 'custom', '1', '> ')).resolves.toBe('> alpha\n> beta');
  await expect(runWasm(page, '> alpha\n> beta', 'outdent', 'custom', '1', '> ')).resolves.toBe('alpha\nbeta');
  await expect(runWasm(page, '    fn main() {\n        ok\n    }', 'dedent', 'spaces', '0')).resolves.toBe('fn main() {\n    ok\n}');
  await expect(runWasm(page, 'first\nsecond', 'indent', 'spaces', '2', '', 'hanging')).resolves.toBe('first\n  second');
  await expect(runWasm(page, 'a\n\nb', 'indent', 'spaces', '2', '', 'all', 'false')).resolves.toBe('  a\n  \n  b');
});

test('indent-block-text page renders output from controls', async ({ page }) => {
  await page.goto('/tools/indent-block-text/');
  await page.fill('#in-text', 'alpha\nbeta');
  await page.selectOption('#in-style', 'custom');
  await page.fill('#in-prefix', '> ');
  await page.fill('#in-count', '1');

  await expect(page.locator('#tool-output')).toContainText('> alpha', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('> beta');
});

test('indent-block-text deep-link prefills dedent parameters and renders', async ({ page }) => {
  const params = new URLSearchParams({
    text: '    x\n    y',
    mode: 'dedent',
    style: 'spaces',
    count: '0',
    prefix: '',
    lines: 'all',
    skip_blank_lines: 'true',
  });

  await page.goto(`/tools/indent-block-text/?${params.toString()}`);
  await expect(page.locator('#in-text')).toHaveValue('    x\n    y', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('dedent');
  await expect(page.locator('#tool-output')).toContainText('x\ny', { timeout: 15_000 });
});
