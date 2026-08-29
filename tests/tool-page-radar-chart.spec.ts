import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setData(page: any, value: string) {
  await page.locator('#in-data').evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('radar-chart page renders real SVG from a wide comparison table', async ({ page }) => {
  await page.goto('/tools/radar-chart/');
  await setData(page, 'product,Camera,Battery,Speed,Price\nPhone A,8,7,9,6\nPhone B,6,9,7,8');
  await page.fill('#in-title', 'Phone comparison');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Phone comparison');
  expect(text).toContain('Camera');
  expect(text).toContain('Phone A');
  expect(text).toContain('Phone B');
  expect(text).toContain('fill="#2563eb"');
  expect(text).toContain('fill="#f97316"');
});

test('radar-chart page deep-link renders percent-scale single-series SVG', async ({ page }) => {
  const data = 'skill,value\nCommunication,85\nDelivery,70\nSystems,90\nMentoring,65';
  await page.goto(
    '/tools/radar-chart/?data=' +
      encodeURIComponent(data) +
      '&layout=single&scale=percent&show_values=true&colors=%2322c55e&title=Candidate%20scorecard&output=svg',
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-layout')).toHaveValue('single');
  await expect(page.locator('#in-scale')).toHaveValue('percent');
  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('Candidate scorecard');
  expect(text).toContain('Communication');
  expect(text).toContain('>85</text>');
  expect(text).toContain('fill="#22c55e"');
});

test('radar-chart page advertised controls render summary and non-default checkbox states', async ({ page }) => {
  await page.goto('/tools/radar-chart/');
  await setData(page, 'series,axis,value\nA,Revenue,50000\nA,Rating,4\nA,Uptime,99\nB,Revenue,25000\nB,Rating,2\nB,Uptime,95');
  await page.selectOption('#in-layout', 'long');
  await page.selectOption('#in-scale', 'per_axis');
  await page.selectOption('#in-grid_shape', 'circle');
  await page.selectOption('#in-direction', 'counterclockwise');
  await page.selectOption('#in-palette', 'ocean');
  await page.selectOption('#in-theme', 'dark');
  await page.selectOption('#in-output', 'summary');
  await page.uncheck('#in-show_spokes');
  await page.uncheck('#in-legend');
  await page.fill('#in-rings', '4');
  await page.fill('#in-fill_opacity', '0.4');
  await page.fill('#in-line_width', '3');
  await page.fill('#in-point_radius', '0');
  await page.fill('#in-start_angle', '45');

  await expect(page.locator('#tool-output')).toContainText('layout: long', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('scale: per_axis');
  expect(text).toContain('Revenue');
  expect(text).toContain('per-axis maximum');
});

test('radar-chart page cap boundary and invalid value error', async ({ page }) => {
  await page.goto('/tools/radar-chart/');
  const axes = Array.from({ length: 60 }, (_, i) => `A${i + 1}`).join(',');
  const vals = Array.from({ length: 60 }, () => '1').join(',');
  await setData(page, `s,${axes}\nOne,${vals}`);
  await page.selectOption('#in-output', 'summary');
  await expect(page.locator('#tool-output')).toContainText('axes: 60', { timeout: 15000 });

  await setData(page, 's,A,B,C\nOne,1,two,3');
  await expect(page.locator('#tool-output')).toContainText('expected a number', { timeout: 15000 });
});
