import { test, expect } from './fixtures';

const sample = '12\n15\n15\n17\n18\n18\n19\n21\n24\n28\n31\n33\n34\n34\n36\n39\n41\n44\n48\n55';

async function runWasm(
  page: import('@playwright/test').Page,
  overrides: Partial<Record<string, string>> = {},
) {
  const args = {
    data: sample,
    binMethod: 'width',
    bins: '10',
    binWidth: '10',
    rangeMin: '0',
    rangeMax: '60',
    normalize: 'count',
    rightClosed: 'false',
    showValues: 'false',
    showMean: 'false',
    showMedian: 'false',
    normalCurve: 'false',
    rug: 'false',
    grid: 'true',
    orientation: 'vertical',
    title: 'Response times',
    xLabel: 'Latency (ms)',
    yLabel: 'Requests',
    width: '800',
    height: '480',
    color: '#2563eb',
    opacity: '0.9',
    theme: 'light',
    precision: '4',
    output: 'svg',
    ...overrides,
  };
  return page.evaluate(async (args) => {
    const mod = await import('/tools/histogram-chart/gizza_ai_histogram_chart_web.js');
    await mod.default('/tools/histogram-chart/gizza_ai_histogram_chart_web_bg.wasm');
    return mod.run(
      args.data,
      args.binMethod,
      args.bins,
      args.binWidth,
      args.rangeMin,
      args.rangeMax,
      args.normalize,
      args.rightClosed,
      args.showValues,
      args.showMean,
      args.showMedian,
      args.normalCurve,
      args.rug,
      args.grid,
      args.orientation,
      args.title,
      args.xLabel,
      args.yLabel,
      args.width,
      args.height,
      args.color,
      args.opacity,
      args.theme,
      args.precision,
      args.output,
    );
  }, args);
}

test('histogram-chart page renders a real SVG histogram from form values', async ({ page }) => {
  await page.goto('/tools/histogram-chart/');
  await page.fill('#in-data', sample);
  await page.selectOption('#in-bin_method', 'width');
  await page.fill('#in-bin_width', '10');
  await page.fill('#in-range_min', '0');
  await page.fill('#in-range_max', '60');
  await page.fill('#in-title', 'Response times');
  await page.fill('#in-x_label', 'Latency (ms)');
  await page.fill('#in-y_label', 'Requests');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15_000 });
  await expect(out).toContainText('Response times');
  await expect(out).toContainText('[10, 20)');
  await expect(out).toContainText('[50, 60]');
  await expect(out).toContainText('[10, 20): 7'); // tallest bin is present in SVG metadata
});

test('histogram-chart deep-link drives table output, exact width bins, and a non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    data: sample,
    bin_method: 'width',
    bin_width: '10',
    range_min: '0',
    range_max: '60',
    output: 'table',
    right_closed: 'true',
    precision: '0',
  });
  await page.goto(`/tools/histogram-chart/?${params.toString()}`);
  await expect(page.locator('#in-bin_method')).toHaveValue('width');
  await expect(page.locator('#in-output')).toHaveValue('table');
  await expect(page.locator('#in-right_closed')).toBeChecked();
  await expect(page.locator('#in-precision')).toHaveValue('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('bins: 6 (width rule), width 10', { timeout: 15_000 });
  await expect(out).toContainText('(10, 20]  7      35       35            7');
  await expect(out).toContainText('(50, 60]   1      5        100           1');
  await expect(out).toContainText('n: 20');
});

test('histogram-chart wasm covers advertised enum choices, boundaries, and styling options', async ({ page }) => {
  await page.goto('/tools/histogram-chart/');
  await page.waitForSelector('#in-data');

  const svg = await runWasm(page, {
    binMethod: 'count',
    bins: '1',
    showValues: 'true',
    showMean: 'true',
    showMedian: 'true',
    normalCurve: 'true',
    rug: 'true',
    orientation: 'horizontal',
    theme: 'dark',
    color: '#f00',
    opacity: '0.05',
    width: '320',
    height: '240',
    precision: '12',
  });
  expect(svg).toContain('<svg');
  expect(svg).toContain('width="320"');
  expect(svg).toContain('height="240"');
  expect(svg).toContain('fill="#f00"');
  expect(svg).toContain('opacity="0.05"');
  expect(svg).toContain('mean 29.1');
  expect(svg).toContain('median 29.5');

  const csv = await runWasm(page, { output: 'csv', binMethod: 'count', bins: '2' });
  expect(csv.split('\n')[0]).toBe('bin,lower,upper,count,percent,cumulative_count,cumulative_percent,value');
  expect(csv).toContain('"[0, 30)",0,30,10,50,10,50,10');

  const json = JSON.parse(await runWasm(page, { output: 'json', normalize: 'percent', binMethod: 'count', bins: '2' }));
  expect(json.bin_count).toBe(2);
  expect(json.normalize).toBe('percent');
  expect(json.bins[0].count).toBe(10);
  expect(json.bins[0].value).toBe(50);

  await expect(runWasm(page, { output: 'nope' })).rejects.toThrow(/unknown output/);
});

test('histogram-chart generated CLI example is generic and brand-free', async ({ page }) => {
  await page.goto('/tools/histogram-chart/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool histogram-chart');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
