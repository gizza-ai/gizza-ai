import { test, expect } from './fixtures';

const tool = '/tools/path-extractor/';
const sample =
  'error[E0308]: mismatched types\n' +
  '  --> src/main.rs:42:9\n' +
  'warning: unused import in src/main.rs\n' +
  '   Compiling foo (/home/dev/projects/foo)';

async function runTool(
  page,
  params: {
    text?: string;
    path_style?: string;
    require_separator?: string;
    keep_line_numbers?: string;
    output?: string;
    extensions?: string;
    extension_mode?: string;
    dedupe?: string;
    sort?: string;
    format?: string;
  } = {},
) {
  const p = {
    text: sample,
    path_style: 'any',
    require_separator: 'true',
    keep_line_numbers: 'false',
    output: 'path',
    extensions: '',
    extension_mode: 'include',
    dedupe: 'true',
    sort: 'first-seen',
    format: 'list',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/path-extractor/gizza_ai_path_extractor_web.js');
    await mod.default('/tools/path-extractor/gizza_ai_path_extractor_web_bg.wasm');
    return mod.run_extract(
      args.text,
      args.path_style,
      args.require_separator,
      args.keep_line_numbers,
      args.output,
      args.extensions,
      args.extension_mode,
      args.dedupe,
      args.sort,
      args.format,
    );
  }, p);
}

test('path-extractor page extracts exact default path list', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', sample);
  await page.selectOption('#in-path_style', 'any');
  await page.selectOption('#in-output', 'path');
  await page.selectOption('#in-format', 'list');

  await expect(page.locator('#tool-output')).toHaveText('src/main.rs\n/home/dev/projects/foo', {
    timeout: 15_000,
  });
});

test('path-extractor deep-link pre-fills filters and renders JSON', async ({ page }) => {
  const qs = new URLSearchParams({
    text: 'open C:\\Users\\dev\\app.log and \\\\srv\\share\\report.csv and src/main.rs',
    path_style: 'windows',
    require_separator: 'true',
    keep_line_numbers: 'true',
    output: 'path',
    extensions: '',
    extension_mode: 'include',
    dedupe: 'true',
    sort: 'first-seen',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-path_style')).toHaveValue('windows', { timeout: 15_000 });
  await expect(page.locator('#in-require_separator')).toBeChecked();
  await expect(page.locator('#in-keep_line_numbers')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count": 2', { timeout: 15_000 });
  await expect(out).toContainText('"path": "C:\\\\Users\\\\dev\\\\app.log"');
  await expect(out).toContainText('"path": "\\\\\\\\srv\\\\share\\\\report.csv"');
  await expect(out).not.toContainText('src/main.rs');
});

test('path-extractor wasm covers advertised modes, caps, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-text');

  await expect(runTool(page)).resolves.toBe('src/main.rs\n/home/dev/projects/foo');
  await expect(runTool(page, { keep_line_numbers: 'true' })).resolves.toBe('src/main.rs:42:9\nsrc/main.rs\n/home/dev/projects/foo');
  await expect(runTool(page, { output: 'filename' })).resolves.toBe('main.rs\nfoo');
  await expect(runTool(page, { output: 'directory' })).resolves.toBe('src\n/home/dev/projects');

  const bareCsv = await runTool(page, {
    text: 'rebuilt main.rs and Cargo.toml, version 1.2.3, pi is 3.14',
    require_separator: 'false',
    extensions: 'rs, toml',
    format: 'csv',
  });
  expect(bareCsv).toBe('path,occurrences\nmain.rs,1\nCargo.toml,1');

  const sorted = await runTool(page, {
    text: 'z/b.txt a/A.txt m/c.txt',
    sort: 'asc',
  });
  expect(sorted).toBe('a/A.txt\nm/c.txt\nz/b.txt');

  const exclude = await runTool(page, {
    text: 'src/a.rs src/b.toml docs/c.md',
    extensions: 'rs md',
    extension_mode: 'exclude',
  });
  expect(exclude).toBe('src/b.toml');

  await expect(runTool(page, { path_style: 'macos' })).rejects.toThrow(/path_style must be one of/);
  await expect(runTool(page, { format: 'yaml' })).rejects.toThrow(/format must be one of/);

  const max = 'src/main.rs ' + 'x'.repeat(1_000_000 - 'src/main.rs '.length);
  await expect(runTool(page, { text: max })).resolves.toBe('src/main.rs');
  await expect(runTool(page, { text: max + 'x' })).rejects.toThrow(/limit is 1000000 bytes/);
});

test('path-extractor ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Rust build log',
    'Git status filenames',
    'Windows paths as JSON',
    'Bare filenames opt-in',
  ]);
});
