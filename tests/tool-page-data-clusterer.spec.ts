import { test, expect } from './fixtures';

const BLOBS = 'x,y\n0,0\n0.2,0.1\n0.1,0.2\n5,5\n5.2,5.1\n4.9,5.2';
const DBSCAN = 'x,y\n0,0\n0.2,0.1\n0.1,0.2\n0,0.3\n5,5\n5.1,5\n5,5.1\n4.9,5.1\n20,20';

test('data-clusterer page renders KMeans SVG clusters', async ({ page }) => {
  await page.goto('/tools/data-clusterer/');
  await page.fill('#in-data', BLOBS);
  await page.fill('#in-clusters', '2');
  await page.fill('#in-title', 'Two blobs');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('Two blobs');
  await expect(out).toContainText('Cluster 1');
  await expect(out).toContainText('Cluster 2');
  await expect(out).toContainText('</svg>');
});

test('data-clusterer page emits CSV labels and supports non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/data-clusterer/');
  await page.uncheck('#in-normalize');
  await page.selectOption('#in-output', 'csv');
  await page.fill('#in-data', BLOBS);
  await page.fill('#in-clusters', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('cluster', { timeout: 15000 });
  await expect(out).toContainText('0,0,cluster 1');
  await expect(out).toContainText('4.9,5.2,cluster 2');
  await expect(page.locator('#in-normalize')).not.toBeChecked();
});

test('data-clusterer page runs DBSCAN and labels noise', async ({ page }) => {
  await page.goto('/tools/data-clusterer/');
  await page.selectOption('#in-method', 'dbscan');
  await page.uncheck('#in-normalize');
  await page.fill('#in-eps', '0.5');
  await page.fill('#in-min_samples', '3');
  await page.fill('#in-data', DBSCAN);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('Noise');
});

test('data-clusterer deep-link pre-fills hierarchical JSON settings', async ({ page }) => {
  await page.goto('/tools/data-clusterer/?method=hierarchical&clusters=2&linkage=ward&output=json&normalize=false&columns=x%2Cy');
  await expect(page.locator('#in-method')).toHaveValue('hierarchical');
  await expect(page.locator('#in-clusters')).toHaveValue('2');
  await expect(page.locator('#in-linkage')).toHaveValue('ward');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-normalize')).not.toBeChecked();
  await expect(page.locator('#in-columns')).toHaveValue('x,y');
  await page.fill('#in-data', BLOBS);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"method": "hierarchical"', { timeout: 15000 });
  await expect(out).toContainText('"clusters": 2');
});
