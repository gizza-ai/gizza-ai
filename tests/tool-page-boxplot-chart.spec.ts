import { test, expect } from './fixtures';

const GROUPED = `group,value
A,1
A,2
A,3
A,4
B,5
B,6
B,7
B,20`;

const SUMMARY_VALUES = `Group   N  Min  Q1    Median  Q3    Max  IQR  Mean     SD       Outliers
------  -  ---  ----  ------  ----  ---  ---  -------  -------  --------
values  6  1    2.25  3.5     4.75  100  2.5  19.1667  39.6253  100

quartile method: linear
whiskers: Tukey, fences at Q1 - 1.5 x IQR and Q3 + 1.5 x IQR`;

test('boxplot-chart renders grouped SVG with labels, color, and title', async ({ page }) => {
  await page.goto('/tools/boxplot-chart/');
  await page.fill('#in-data', GROUPED);
  await page.fill('#in-title', 'Latency by group');
  await page.fill('#in-value_label', 'Latency (ms)');
  await page.fill('#in-group_label', 'Group');
  await page.fill('#in-color', '#dc2626');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg xmlns="http://www.w3.org/2000/svg"', { timeout: 15_000 });
  const svg = (await out.textContent())!;
  expect(svg).toContain('Latency by group');
  expect(svg).toContain('Latency (ms)');
  expect(svg).toContain('>A<');
  expect(svg).toContain('>B<');
  expect(svg).toContain('#dc2626');
  expect(svg).toContain('<circle');
  expect(svg).toContain('</svg>');
});

test('boxplot-chart summary output is exact for a simple value list', async ({ page }) => {
  await page.goto('/tools/boxplot-chart/');
  await page.fill('#in-data', '1\n2\n3\n4\n5\n100');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toHaveText(SUMMARY_VALUES, { timeout: 15_000 });
});

test('boxplot-chart honours deep-link params, non-default checkboxes, and boundary sizes', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'class_a,class_b\n78,82\n83,91\n88,76\n92,99\n,101',
    layout: 'wide',
    quartile_method: 'exclusive',
    whiskers: 'percentile',
    percentile: '49',
    points: 'none',
    show_mean: 'false',
    notched: 'true',
    orientation: 'horizontal',
    grid: 'false',
    width: '320',
    height: '240',
    color: '#f00',
    theme: 'dark',
    output: 'json',
  });

  await page.goto(`/tools/boxplot-chart/?${params.toString()}`);

  await expect(page.locator('#in-layout')).toHaveValue('wide', { timeout: 15_000 });
  await expect(page.locator('#in-quartile_method')).toHaveValue('exclusive');
  await expect(page.locator('#in-whiskers')).toHaveValue('percentile');
  await expect(page.locator('#in-percentile')).toHaveValue('49');
  await expect(page.locator('#in-points')).toHaveValue('none');
  await expect(page.locator('#in-show_mean')).not.toBeChecked();
  await expect(page.locator('#in-notched')).toBeChecked();
  await expect(page.locator('#in-orientation')).toHaveValue('horizontal');
  await expect(page.locator('#in-grid')).not.toBeChecked();
  await expect(page.locator('#in-width')).toHaveValue('320');
  await expect(page.locator('#in-height')).toHaveValue('240');
  await expect(page.locator('#in-color')).toHaveValue('#f00');
  await expect(page.locator('#in-theme')).toHaveValue('dark');
  await expect(page.locator('#in-output')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"quartile_method": "exclusive"', { timeout: 15_000 });
  await expect(out).toContainText('"whiskers": "percentile"');
  await expect(out).toContainText('"name": "class_a"');
  await expect(out).toContainText('"name": "class_b"');
});

test('boxplot-chart covers advertised enum values and checkbox states in SVG mode', async ({ page }) => {
  await page.goto('/tools/boxplot-chart/');
  await page.fill('#in-data', '1\n2\n3\n4\n5\n100');
  await page.selectOption('#in-layout', 'values');
  await page.selectOption('#in-quartile_method', 'inclusive');
  await page.selectOption('#in-whiskers', 'minmax');
  await page.selectOption('#in-points', 'all');
  await page.selectOption('#in-orientation', 'horizontal');
  await page.uncheck('#in-grid');
  await page.uncheck('#in-show_mean');
  await page.check('#in-notched');
  await page.fill('#in-color', 'tomato');
  await page.selectOption('#in-theme', 'dark');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15_000 });
  const svg = (await out.textContent())!;
  expect(svg).toContain('tomato');
  expect(svg).toContain('#0f172a');
  expect(svg).toContain('<polygon');
  expect(svg).toContain('<circle');
});

test('boxplot-chart generated CLI example stays runnable and generic', async ({ page }) => {
  await page.goto('/tools/boxplot-chart/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool boxplot-chart');
  expect(cli).toContain('group,value');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
