import { test, expect } from './fixtures';

const PATHS = `img10.png
img2.png
img1.png
src/main.rs
src/lib.rs
README.md`;

async function runWasm(
  page: any,
  paths: string,
  sortBy = 'natural',
  order = 'asc',
  ignoreCase = 'true',
  dirsFirst = 'true',
  groupByDir = 'false',
  unique = 'false',
  trim = 'true',
  format = 'list',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/file-list-sorter/gizza_ai_file_list_sorter_web.js');
    await mod.default('/tools/file-list-sorter/gizza_ai_file_list_sorter_web_bg.wasm');
    return mod.run(
      args.paths,
      args.sortBy,
      args.order,
      args.ignoreCase,
      args.dirsFirst,
      args.groupByDir,
      args.unique,
      args.trim,
      args.format,
    );
  }, { paths, sortBy, order, ignoreCase, dirsFirst, groupByDir, unique, trim, format });
}

test('file-list-sorter page natural-sorts a pasted list', async ({ page }) => {
  await page.goto('/tools/file-list-sorter/');
  await page.fill('#in-paths', PATHS);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('img1.png', { timeout: 20_000 });
  const text = await output.textContent();
  expect(text!.indexOf('img2.png')).toBeLessThan(text!.indexOf('img10.png'));
  expect(text!.indexOf('src/lib.rs')).toBeLessThan(text!.indexOf('src/main.rs'));
});

test('file-list-sorter deep link sorts by size descending', async ({ page }) => {
  const params = new URLSearchParams({
    paths: '4.0K\tsrc/app.js\n2.1M\tassets/hero.png\n512B\tREADME.md\n18M\tvideo/clip.mp4',
    sort_by: 'size',
    order: 'desc',
    dirs_first: 'false',
    format: 'table',
  });
  await page.goto(`/tools/file-list-sorter/?${params.toString()}`);

  await expect(page.locator('#in-sort_by')).toHaveValue('size', { timeout: 15_000 });
  await expect(page.locator('#in-order')).toHaveValue('desc');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('video/clip.mp4', { timeout: 20_000 });
  const text = await output.textContent();
  expect(text!.indexOf('video/clip.mp4')).toBeLessThan(text!.indexOf('assets/hero.png'));
});

test('file-list-sorter wasm covers sort enums, booleans, errors and CLI example', async ({ page }) => {
  await page.goto('/tools/file-list-sorter/');

  await expect(runWasm(page, PATHS, 'natural')).resolves.toBe('img1.png\nimg2.png\nimg10.png\nREADME.md\nsrc/lib.rs\nsrc/main.rs');
  await expect(runWasm(page, PATHS, 'alpha')).resolves.toContain('img1.png\nimg10.png\nimg2.png');

  const ext = await runWasm(page, 'notes.md\narchive.tar.gz\nsrc/main.rs\nphoto.jpg\nREADME', 'extension', 'asc', 'true', 'true', 'false', 'false', 'true', 'table');
  expect(ext).toContain('archive.tar.gz');
  expect(ext).toContain('photo.jpg');

  const depth = await runWasm(page, 'src/components/ui/button.tsx\npackage.json\nsrc/index.ts', 'depth', 'asc', 'true', 'true', 'false', 'false', 'true', 'numbered');
  expect(depth).toContain('1. package.json');

  const basename = await runWasm(page, 'zzz/apple.txt\naaa/banana.txt', 'basename');
  expect(basename).toBe('zzz/apple.txt\naaa/banana.txt');

  const unique = await runWasm(page, './src/app.js\nsrc/app.js\nSRC/APP.JS', 'natural', 'asc', 'true', 'true', 'false', 'true');
  expect(unique.split('\n')).toHaveLength(1);

  await expect(runWasm(page, 'a.txt\nb.txt', 'size')).rejects.toThrow(/size/);
  await expect(runWasm(page, '', 'natural')).rejects.toThrow(/empty/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool file-list-sorter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
