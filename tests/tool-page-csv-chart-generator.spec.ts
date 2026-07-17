import { test, expect } from './fixtures';

test('csv-chart-generator page — bar chart renders real SVG output', async ({ page }) => {
  await page.goto('/tools/csv-chart-generator/');
  await page.fill('#in-data', 'month,revenue\nJan,1200\nFeb,1800\nMar,1500\nApr,2400');
  await page.selectOption('#in-chart_type', 'bar');
  await page.fill('#in-x_column', 'month');
  await page.fill('#in-y_column', 'revenue');
  await page.fill('#in-title', 'Monthly revenue');
  await page.fill('#in-color', '#dc2626');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const svg = await page.locator('#tool-output').textContent();
  expect(svg).toContain('<svg');
  expect(svg).toContain('Monthly revenue');
  expect(svg).toContain('Jan');
  expect(svg).toContain('Apr');
  expect(svg).toContain('#dc2626');
  expect(svg).toContain('<rect');
});

test('csv-chart-generator page — deep-link renders histogram with requested bins', async ({ page }) => {
  const data = 'score\n55\n61\n62\n67\n70\n71\n72\n73\n74\n78\n80\n82\n85\n88\n91\n95';
  await page.goto(
    `/tools/csv-chart-generator/?data=${encodeURIComponent(data)}&chart_type=histogram&y_column=score&title=${encodeURIComponent('Score distribution')}&bins=6&color=${encodeURIComponent('#f59e0b')}`,
  );

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const svg = await page.locator('#tool-output').textContent();
  expect(svg).toContain('Score distribution');
  expect(svg).toContain('#f59e0b');
  expect(svg).toContain('<rect');
  expect(svg).toContain('55');
  expect(svg).toContain('95');
});
