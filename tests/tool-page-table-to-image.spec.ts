import { test, expect } from './fixtures';

const csv = 'name,score\nAda,9\nGrace,8';

test('table-to-image renders CSV as a deterministic SVG table', async ({ page }) => {
  await page.goto('/tools/table-to-image/');
  await page.fill('#in-input', csv);
  await page.selectOption('#in-input_format', 'csv');
  await page.fill('#in-delimiter', ',');
  await page.check('#in-header');
  await page.check('#in-zebra');
  await page.selectOption('#in-theme', 'light');
  await page.fill('#in-accent', '#2563eb');
  await page.fill('#in-font_size', '14');
  await page.fill('#in-cell_padding', '10');
  await page.fill('#in-title', 'Leaderboard');
  await page.selectOption('#in-align', 'left');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg xmlns="http://www.w3.org/2000/svg"', { timeout: 15000 });
  await expect(out).toContainText('width="156" height="164"');
  await expect(out).toContainText('Leaderboard');
  await expect(out).toContainText('fill="#2563eb"');
  await expect(out).toContainText('>Ada</text>');
  await expect(out).toContainText('>Grace</text>');
});

test('table-to-image renders JSON object arrays with dark theme and centered text', async ({ page }) => {
  await page.goto('/tools/table-to-image/');
  await page.fill('#in-input', '[{"metric":"Revenue","value":"$42k"},{"metric":"Cost","value":"$18k"}]');
  await page.selectOption('#in-input_format', 'json');
  await page.selectOption('#in-theme', 'dark');
  await page.fill('#in-accent', '#7c3aed');
  await page.selectOption('#in-align', 'center');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('fill="#0f172a"', { timeout: 15000 });
  await expect(out).toContainText('fill="#7c3aed"');
  await expect(out).toContainText('text-anchor="middle"');
  await expect(out).toContainText('>metric</text>');
  await expect(out).toContainText('>Revenue</text>');
});

test('table-to-image surfaces invalid JSON errors clearly', async ({ page }) => {
  await page.goto('/tools/table-to-image/');
  await page.fill('#in-input', '{not json');
  await page.selectOption('#in-input_format', 'json');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});

test('table-to-image query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto(
    '/tools/table-to-image/?input=' +
      encodeURIComponent(csv) +
      '&input_format=csv&delimiter=%2C&header=true&zebra=true&theme=light&accent=%232563eb&font_size=14&cell_padding=10&title=' +
      encodeURIComponent('Leaderboard') +
      '&align=left',
  );
  await expect(page.locator('#in-input')).toHaveValue(csv, { timeout: 15000 });
  await expect(page.locator('#in-title')).toHaveValue('Leaderboard');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg xmlns="http://www.w3.org/2000/svg"', { timeout: 15000 });
  await expect(out).toContainText('Leaderboard');
  await expect(out).toContainText('>Ada</text>');
});
