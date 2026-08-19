import { test, expect } from './fixtures';

async function runWasm(
  page,
  data: string,
  mode = 'maxima',
  separator = 'auto',
  smooth = '0',
  minValue = '',
  maxValue = '',
  threshold = '0',
  minDistance = '0',
  minProminence = '0',
  minWidth = '0',
  relHeight = '0.5',
  maxPeaks = '0',
  sortBy = 'position',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/peak-detector/gizza_ai_peak_detector_web.js');
    await mod.default('/tools/peak-detector/gizza_ai_peak_detector_web_bg.wasm');
    return mod.run(
      args.data,
      args.mode,
      args.separator,
      args.smooth,
      args.minValue,
      args.maxValue,
      args.threshold,
      args.minDistance,
      args.minProminence,
      args.minWidth,
      args.relHeight,
      args.maxPeaks,
      args.sortBy,
      args.format,
    );
  }, { data, mode, separator, smooth, minValue, maxValue, threshold, minDistance, minProminence, minWidth, relHeight, maxPeaks, sortBy, format });
}

test('peak-detector wasm finds maxima with exact indices and prominence', async ({ page }) => {
  await page.goto('/tools/peak-detector/');
  await page.waitForSelector('#in-data');

  const report = await runWasm(page, '0, 1, 0, 3, 0, 2, 0');
  expect(report).toContain('3 maxima found in 7 values.');
  expect(report).toContain('Highest peak: index 3, value 3 (prominence 3)');
  expect(report).toContain('Mean spacing: 2.00 samples');
});

test('peak-detector wasm covers modes, formats, sorting, caps, and validation', async ({ page }) => {
  await page.goto('/tools/peak-detector/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, '5, 1, 5, 2, 5', 'minima')).resolves.toContain('2 minima found in 5 values.');
  await expect(runWasm(page, '0, 2, 0, 9, 0, 5, 0', 'maxima', 'auto', '0', '', '', '0', '0', '0', '0', '0.5', '2', 'prominence'))
    .resolves.toContain('Showing 2 of 3 matching peaks (max_peaks=2)');

  const json = JSON.parse(await runWasm(page, '0 1 0 3 0 2 0', 'maxima', 'space', '0', '', '', '0', '0', '0', '0', '0.5', '0', 'position', 'json'));
  expect(json.count).toBe(3);
  expect(json.peaks[1]).toMatchObject({ kind: 'maximum', index: 3, value: 3 });

  const csv = await runWasm(page, '0\n1\n0\n3\n0', 'both', 'newline', '0', '', '', '0', '0', '0', '0', '0.5', '0', 'position', 'csv');
  expect(csv).toContain('kind,index,value,prominence,width');
  expect(csv).toContain('minimum,2,0');

  await expect(runWasm(page, '0,1,0', 'maxima', 'auto', '0', '', '', '0', '0', '0', '0', '1.1'))
    .rejects.toThrow(/rel_height must be between 0 and 1/);
});

test('peak-detector page renders output from controls', async ({ page }) => {
  await page.goto('/tools/peak-detector/');
  await page.fill('#in-data', '0, 1, 0, 3, 0, 2, 0');
  await page.selectOption('#in-mode', 'maxima');
  await page.fill('#in-max_peaks', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 maxima found in 7 values.', { timeout: 15_000 });
  await expect(out).toContainText('Highest peak: index 3, value 3');
});

test('peak-detector deep-link prefills filters and renders csv output', async ({ page }) => {
  const params = new URLSearchParams({
    data: '0\n1\n0\n3\n0',
    mode: 'both',
    separator: 'newline',
    smooth: '0',
    min_value: '',
    max_value: '',
    threshold: '0',
    min_distance: '0',
    min_prominence: '0',
    min_width: '0',
    rel_height: '0.5',
    max_peaks: '0',
    sort_by: 'position',
    format: 'csv',
  });

  await page.goto(`/tools/peak-detector/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue('0\n1\n0\n3\n0', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('both');
  await expect(page.locator('#in-separator')).toHaveValue('newline');
  await expect(page.locator('#in-format')).toHaveValue('csv');

  await expect(page.locator('#tool-output')).toContainText('kind,index,value,prominence,width', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('minimum,2,0');
});
