import { test, expect } from './fixtures';

test('graph-algorithms BFS traversal', async ({ page }) => {
  await page.goto('/tools/graph-algorithms/');
  await page.fill('#in-edges', 'a - b\na - c\nb - d');
  // Default algorithm is bfs.
  await page.fill('#in-start', 'a');
  await expect(page.locator('#tool-output')).toContainText('a → b → c → d', {
    timeout: 15000,
  });
});

test('graph-algorithms DFS goes deep first', async ({ page }) => {
  await page.goto('/tools/graph-algorithms/');
  await page.fill('#in-edges', 'a - b\na - c\nb - d');
  await page.selectOption('#in-algorithm', 'dfs');
  await page.fill('#in-start', 'a');
  await expect(page.locator('#tool-output')).toContainText('a → b → d → c', {
    timeout: 15000,
  });
});

test('graph-algorithms weighted shortest path uses Dijkstra', async ({ page }) => {
  await page.goto('/tools/graph-algorithms/');
  await page.fill('#in-edges', 'a -> b : 1\na -> c : 5\nb -> c : 1');
  await page.selectOption('#in-algorithm', 'shortest-path');
  await page.check('#in-directed');
  await page.check('#in-weighted');
  await page.fill('#in-start', 'a');
  await page.fill('#in-end', 'c');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a → b → c', { timeout: 15000 });
  await expect(out).toContainText('total distance: 2');
});

test('graph-algorithms cycle detection finds a cycle', async ({ page }) => {
  await page.goto('/tools/graph-algorithms/');
  await page.fill('#in-edges', 'a -> b\nb -> c\nc -> a');
  await page.selectOption('#in-algorithm', 'cycle-detection');
  await page.check('#in-directed');
  await expect(page.locator('#tool-output')).toContainText('a cycle was found', {
    timeout: 15000,
  });
});

test('graph-algorithms query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto(
    '/tools/graph-algorithms/?edges=' +
      encodeURIComponent('a -> b\nb -> c\nc -> d') +
      '&algorithm=topological-sort&directed=true',
  );
  await expect(page.locator('#in-algorithm')).toHaveValue('topological-sort', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('a → b → c → d', {
    timeout: 15000,
  });
});
