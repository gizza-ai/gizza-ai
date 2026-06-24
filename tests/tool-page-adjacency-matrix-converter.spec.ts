import { test, expect } from './fixtures';

test('adjacency-matrix-converter edges -> adjacency (default)', async ({ page }) => {
  await page.goto('/tools/adjacency-matrix-converter/');
  await page.fill('#in-input', 'A B\nB C\nA C');
  // Default from=edges, to=adjacency, undirected. Symmetric triangle.
  const out = page.locator('#tool-output');
  // toHaveText normalizes whitespace, so tabs collapse to single spaces.
  await expect(out).toHaveText('A B C A 0 1 1 B 1 0 1 C 1 1 0', {
    timeout: 15000,
  });
});

test('adjacency-matrix-converter edges -> incidence directed', async ({ page }) => {
  await page.goto('/tools/adjacency-matrix-converter/');
  await page.fill('#in-input', 'A B\nB C');
  await page.selectOption('#in-to', 'incidence');
  await page.check('#in-directed');
  // Directed incidence: tail -1, head +1.
  await expect(page.locator('#tool-output')).toHaveText(
    'e1 e2 A -1 0 B 1 -1 C 0 1',
    { timeout: 15000 },
  );
});

test('adjacency-matrix-converter adjacency -> edges', async ({ page }) => {
  await page.goto('/tools/adjacency-matrix-converter/');
  await page.fill('#in-input', ' \tA\tB\tC\nA\t0\t1\t1\nB\t1\t0\t0\nC\t1\t0\t0');
  await page.selectOption('#in-from', 'adjacency');
  await page.selectOption('#in-to', 'edges');
  await expect(page.locator('#tool-output')).toHaveText('A B A C', {
    timeout: 15000,
  });
});

test('adjacency-matrix-converter edges -> laplacian', async ({ page }) => {
  await page.goto('/tools/adjacency-matrix-converter/');
  await page.fill('#in-input', 'A B\nB C');
  await page.selectOption('#in-to', 'laplacian');
  // Path A-B-C: L = D - A.
  await expect(page.locator('#tool-output')).toHaveText(
    'A B C A 1 -1 0 B -1 2 -1 C 0 -1 1',
    { timeout: 15000 },
  );
});

test('adjacency-matrix-converter query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/adjacency-matrix-converter/?input=' +
      encodeURIComponent('A B 5\nB C 2.5') +
      '&weighted=true',
  );
  await expect(page.locator('#in-input')).toHaveValue('A B 5\nB C 2.5', {
    timeout: 15000,
  });
  // Weighted adjacency carries the weights.
  await expect(page.locator('#tool-output')).toHaveText(
    'A B C A 0 5 0 B 5 0 2.5 C 0 2.5 0',
    { timeout: 15000 },
  );
});
