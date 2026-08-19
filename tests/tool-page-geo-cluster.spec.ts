import { test, expect } from './fixtures';

const points = [
  '51.5074,-0.1278,Charing Cross',
  '51.5079,-0.1281,Nelson Column',
  '51.5071,-0.1274,Strand',
  '51.5155,-0.1410,Oxford Circus',
  '51.5161,-0.1415,Regent Street',
  '40.7128,-74.0060,New York',
].join('\n');

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('geo-cluster page clusters nearby London points and leaves New York as noise', async ({ page }) => {
  await page.goto('/tools/geo-cluster/');
  await page.fill('#in-points', points);
  await page.selectOption('#in-method', 'dbscan');
  await page.fill('#in-radius', '200');
  await page.selectOption('#in-units', 'm');
  await page.fill('#in-min_points', '2');
  await page.selectOption('#in-output', 'text');

  await expect(page.locator('#tool-output')).toContainText('6 points', { timeout: 15_000 });
  const text = await output(page);
  expect(text).toContain('2 clusters');
  expect(text).toContain('1 unclustered');
  expect(text).toContain('centroid 51.507467, -0.127767');
});

test('geo-cluster deep link can render CSV grid output', async ({ page }) => {
  const qs = new URLSearchParams({
    points,
    method: 'grid',
    radius: '1',
    units: 'km',
    min_points: '1',
    coord_order: 'lat_lon',
    output: 'csv',
  });
  await page.goto(`/tools/geo-cluster/?${qs.toString()}`);

  await expect(page.locator('#in-method')).toHaveValue('grid', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('point,latitude,longitude,label,cluster', { timeout: 15_000 });
  const text = await output(page);
  expect(text).toContain('Charing Cross');
  expect(text).toContain('Oxford Circus');
});

test('geo-cluster reports out-of-range coordinates', async ({ page }) => {
  await page.goto('/tools/geo-cluster/');
  await page.fill('#in-points', '120,0,bad');

  await expect(page.locator('#tool-output')).toContainText('latitude', { timeout: 15_000 });
});
