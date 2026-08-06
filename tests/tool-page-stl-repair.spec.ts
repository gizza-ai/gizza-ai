import { test, expect } from './fixtures';

const TETRA_STL = `solid tetra
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 0 1
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 0 0 0
vertex 0 0 1
vertex 0 1 0
endloop
endfacet
facet normal 0 0 0
outer loop
vertex 1 0 0
vertex 0 1 0
vertex 0 0 1
endloop
endfacet
endsolid tetra`;

const EXPECTED_REPORT = `STL repair report
=================

Input
  Format                  ASCII STL
  Solid name              tetra
  Triangles               4
  Distinct vertices       4

Problems found
  Degenerate triangles    0
  Duplicate triangles     0
  Coincident vertices     0
  Non-manifold edges      0
  Open (boundary) edges   0
  Flipped triangles       3
  Disconnected shells     1
  Watertight              no

Repairs applied
  Vertices welded         0
  Degenerate removed      0
  Duplicates removed      0
  Triangles re-wound      1
  Shells turned outward   0
  Holes filled            0 (0 triangles added)
  Fragments removed       0 (0 triangles dropped)

Result
  Triangles               4
  Distinct vertices       4
  Non-manifold edges      0
  Open (boundary) edges   0
  Disconnected shells     1
  Watertight              yes
  Surface area            2.366025
  Volume                  0.166667
  Bounding box            1 x 1 x 1
  Bounds                  min 0 0 0 / max 1 1 1

Lengths, areas and volumes are in the mesh's own units (STL stores no unit).`;

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('stl-repair page renders the exact repair report', async ({ page }) => {
  await page.goto('/tools/stl-repair/');
  await page.fill('#in-stl', TETRA_STL);
  await expect(page.locator('#tool-output')).toContainText('STL repair report', { timeout: 15000 });
  expect(await output(page)).toBe(EXPECTED_REPORT);
});

test('stl-repair deep link pre-fills and returns repaired STL text', async ({ page }) => {
  await page.goto(
    `/tools/stl-repair/?stl=${encodeURIComponent(TETRA_STL)}&output=stl&stl_encoding=ascii`,
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('solid tetra', { timeout: 15000 });
  await expect(out).toContainText('facet normal');
  await expect(out).toContainText('endsolid tetra');
});

test('stl-repair invalid mesh shows actionable error', async ({ page }) => {
  await page.goto('/tools/stl-repair/?stl=not-a-mesh&output=report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('could not detect the mesh format', { timeout: 15000 });
  await expect(out).toContainText('Binary STL cannot be pasted as text');
});
