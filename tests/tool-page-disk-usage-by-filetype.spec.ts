import { test, expect } from './fixtures';

const SAMPLE = `4096\t./src/app.js
2097152\t./assets/hero.png
1048576\t./assets/logo.png
18874368\t./media/clip.mp4`;

const CHART = `Disk usage by extension — 4 file(s), 21.0 MiB total

.mp4  18.0 MiB   85.7%  ████████████████████████████████  1 file(s)
.png   3.0 MiB   14.3%  █████▍                            2 file(s)
.js    4.0 KiB    0.0%  ▏                                 1 file(s)
`;

async function runWasm(
  page: import('@playwright/test').Page,
  overrides: Partial<Record<string, string>> = {},
) {
  const args = {
    listing: SAMPLE,
    groupBy: 'extension',
    sortBy: 'size',
    order: 'desc',
    topN: '15',
    units: 'binary',
    chartWidth: '32',
    skipFolders: 'true',
    ignoreCase: 'true',
    format: 'chart',
    ...overrides,
  };
  return page.evaluate(async (args) => {
    const mod = await import('/tools/disk-usage-by-filetype/gizza_ai_disk_usage_by_filetype_web.js');
    await mod.default('/tools/disk-usage-by-filetype/gizza_ai_disk_usage_by_filetype_web_bg.wasm');
    return mod.run(
      args.listing,
      args.groupBy,
      args.sortBy,
      args.order,
      args.topN,
      args.units,
      args.chartWidth,
      args.skipFolders,
      args.ignoreCase,
      args.format,
    );
  }, args);
}

test('disk-usage-by-filetype page renders exact chart from form values', async ({ page }) => {
  await page.goto('/tools/disk-usage-by-filetype/');
  await page.fill('#in-listing', SAMPLE);
  await page.fill('#in-top_n', '5');

  await expect(page.locator('#tool-output')).toContainText('Disk usage by extension', { timeout: 15_000 });
  expect(await page.locator('#tool-output').textContent()).toBe(CHART);
});

test('disk-usage-by-filetype deep link drives category table and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    listing: `4096\t./src/app.js\n2097152\t./assets/hero.png\n1048576\t./assets/logo.png\n18874368\t./media/clip.mp4`,
    group_by: 'category',
    format: 'table',
    units: 'si',
    skip_folders: 'false',
  });
  await page.goto(`/tools/disk-usage-by-filetype/?${params.toString()}`);

  await expect(page.locator('#in-group_by')).toHaveValue('category');
  await expect(page.locator('#in-format')).toHaveValue('table');
  await expect(page.locator('#in-units')).toHaveValue('si');
  await expect(page.locator('#in-skip_folders')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('Category', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('video');
  await expect(page.locator('#tool-output')).toContainText('images');
  await expect(page.locator('#tool-output')).toContainText('TOTAL');
});

test('disk-usage-by-filetype wasm covers advertised options and boundaries', async ({ page }) => {
  await page.goto('/tools/disk-usage-by-filetype/');
  await page.waitForSelector('#in-listing');

  expect(await runWasm(page, { topN: '5' })).toBe(CHART);

  const csv = await runWasm(page, { sortBy: 'count', format: 'csv', units: 'bytes', chartWidth: '8' });
  expect(csv.split('\n')[0]).toBe('extension,bytes,size,percent,files');
  expect(csv).toContain('.png,3145728,3145728,14.3,2');

  const json = JSON.parse(await runWasm(page, { groupBy: 'category', format: 'json', skipFolders: 'false', topN: '200' }));
  expect(json.group_by).toBe('category');
  expect(json.total_bytes).toBe(22024192);
  expect(json.groups.map((g: { name: string }) => g.name)).toContain('video');
  expect(json.groups.map((g: { name: string }) => g.name)).toContain('images');

  const svg = await runWasm(page, { format: 'svg', order: 'asc', ignoreCase: 'false', chartWidth: '120' });
  expect(svg).toContain('<svg');
  expect(svg).toContain('Disk usage by extension');

  await expect(runWasm(page, { format: 'nope' })).rejects.toThrow(/invalid format/);
  await expect(runWasm(page, { topN: 'not-a-number' })).rejects.toThrow(/top_n must be a whole number/);
});

test('disk-usage-by-filetype generated CLI example is generic and brand-free', async ({ page }) => {
  await page.goto('/tools/disk-usage-by-filetype/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool disk-usage-by-filetype');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
