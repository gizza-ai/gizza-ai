import { test, expect } from './fixtures';

test('pie-donut-chart-svg page — pie chart renders real SVG output', async ({ page }) => {
  await page.goto('/tools/pie-donut-chart-svg/');
  await page.fill('#in-data', 'Chrome, 63\nSafari, 20\nEdge, 5\nFirefox, 3\nOther, 9');
  await page.selectOption('#in-chart_type', 'pie');
  await page.fill('#in-title', 'Browser market share');
  await page.fill('#in-colors', '#dc2626, #2563eb, #16a34a, #f59e0b, #7c3aed');
  await page.selectOption('#in-sort', 'descending');

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const svg = await page.locator('#tool-output').textContent();
  expect(svg).toContain('<svg');
  expect(svg).toContain('Browser market share');
  expect(svg).toContain('Chrome');
  expect(svg).toContain('#dc2626');
  expect(svg).toContain('<path');
});

test('pie-donut-chart-svg page — deep-link renders a donut with a hole', async ({ page }) => {
  const data = 'Rent, 1400\nFood, 600\nTransport, 250\nSavings, 500';
  await page.goto(
    `/tools/pie-donut-chart-svg/?data=${encodeURIComponent(data)}&chart_type=donut&donut_hole=0.6&legend=bottom&show_values=true&title=${encodeURIComponent('Monthly budget')}`,
  );

  await expect(page.locator('#tool-output')).toContainText('<svg', { timeout: 15000 });
  const svg = await page.locator('#tool-output').textContent();
  expect(svg).toContain('Monthly budget');
  expect(svg).toContain('Rent');
  expect(svg).toContain('<path');
  // donut annular slices use an inner arc — at least two "A" arc commands per path.
  expect((svg!.match(/ A/g) || []).length).toBeGreaterThanOrEqual(8);
});
