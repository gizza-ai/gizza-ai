import { test, expect } from './fixtures';

const ASCII_TRIANGLE = `solid tri
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
endsolid tri
`;

const TWO_TRIANGLES = `facet normal 0 0 1
vertex 0 0 0
vertex 1 0 0
vertex 1 1 0
endfacet
facet normal 0 0 1
vertex 0 0 0
vertex 1 1 0
vertex 0 1 0
endfacet
`;

const HEX_TRIANGLE =
  '00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000803f0000000000000000000000000000803f0000000000000000000000000000803f000000000000';

test('stl-vertices-to-csv page extracts default ASCII triangle vertices', async ({ page }) => {
  await page.goto('/tools/stl-vertices-to-csv/');
  await page.fill('#in-stl', ASCII_TRIANGLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('x,y,z', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`x,y,z
0,0,0
1,0,0
0,1,0`);
});

test('stl-vertices-to-csv page deep-link renders triangle rows with computed normals', async ({ page }) => {
  const stl = 'facet normal 0 0 -1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendfacet';
  const qs =
    '?stl=' +
    encodeURIComponent(stl) +
    '&input_format=ascii' +
    '&rows=triangle' +
    '&columns=normals' +
    '&normal_source=computed' +
    '&up_axis=keep' +
    '&scale=1' +
    '&precision=-1' +
    '&dedupe=none' +
    '&every_nth=1' +
    '&delimiter=comma' +
    '&header=true';
  await page.goto('/tools/stl-vertices-to-csv/' + qs);

  await expect(page.locator('#in-rows')).toHaveValue('triangle', { timeout: 15_000 });
  await expect(page.locator('#in-normal_source')).toHaveValue('computed');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('v1x,v1y,v1z', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`v1x,v1y,v1z,v2x,v2y,v2z,v3x,v3y,v3z,nx,ny,nz
0,0,0,1,0,0,0,1,0,0,0,1`);
});

test('stl-vertices-to-csv page emits deduped plain XYZ text', async ({ page }) => {
  await page.goto('/tools/stl-vertices-to-csv/');
  await page.fill('#in-stl', TWO_TRIANGLES);
  await page.selectOption('#in-delimiter', 'space');
  await page.locator('#in-header').uncheck();
  await page.fill('#in-precision', '2');
  await page.selectOption('#in-dedupe', 'all');

  await expect(page.locator('#tool-output')).toHaveText(
    '0.00 0.00 0.00\n1.00 0.00 0.00\n1.00 1.00 0.00\n0.00 1.00 0.00',
    { timeout: 15_000 },
  );
});

test('stl-vertices-to-csv page accepts binary hex STL and indexes corners', async ({ page }) => {
  await page.goto('/tools/stl-vertices-to-csv/');
  await page.fill('#in-stl', HEX_TRIANGLE);
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-columns', 'indexed');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('triangle,corner,x,y,z', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`triangle,corner,x,y,z
1,1,0,0,0
1,2,1,0,0
1,3,0,1,0`);
});

test('stl-vertices-to-csv page thins every nth surviving row and converts axes', async ({ page }) => {
  await page.goto('/tools/stl-vertices-to-csv/');
  await page.fill('#in-stl', ASCII_TRIANGLE);
  await page.selectOption('#in-up_axis', 'z-to-y');
  await page.fill('#in-scale', '10');
  await page.fill('#in-every_nth', '2');
  await page.selectOption('#in-columns', 'full');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('triangle,corner,x,y,z,nx,ny,nz', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`triangle,corner,x,y,z,nx,ny,nz
1,1,0,0,0,0,1,0
1,3,0,0,-10,0,1,0`);
});
