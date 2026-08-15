import { test, expect } from './fixtures';

const ASCII_TETRA = `solid tetra
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 10 0
      vertex 10 0 0
    endloop
  endfacet
  facet normal 0 -1 0
    outer loop
      vertex 0 0 0
      vertex 10 0 0
      vertex 0 0 10
    endloop
  endfacet
  facet normal -1 0 0
    outer loop
      vertex 0 0 0
      vertex 0 0 10
      vertex 0 10 0
    endloop
  endfacet
  facet normal 0.5773502691896257 0.5773502691896257 0.5773502691896257
    outer loop
      vertex 10 0 0
      vertex 0 10 0
      vertex 0 0 10
    endloop
  endfacet
endsolid tetra
`;

const SINGLE_TRIANGLE = `solid demo
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 10 0
      vertex 10 0 0
    endloop
  endfacet
endsolid demo
`;

const TETRA_REPORT = [
  'STL inspection report',
  '=====================',
  '',
  'Input',
  '  Encoding              ASCII STL (text, auto-detected)',
  '  Solid name / header   tetra',
  '  Units                 mm',
  '  Scale factor          1',
  '',
  'Geometry',
  '  Triangles             4',
  '  Distinct vertices     4',
  '  Bounding box          10 x 10 x 10 mm',
  '  Bounds                min 0 0 0 / max 10 10 10 mm',
  '  Center                5 5 5 mm',
  '  Surface area          236.60254 mm²',
  '  Volume                166.666667 mm³ (0.166667 cm³)',
  '  Signed volume         166.666667 mm³ (positive — normals face outward)',
  '',
  'Mesh integrity',
  '  Watertight            yes',
  '  Manifold              yes',
  '  Open (boundary) edges 0',
  '  Non-manifold edges    0',
  '  Inconsistent winding  0',
  '  Disconnected shells   1',
  '  Degenerate triangles  0',
  '  Duplicate triangles   0',
  '  Normals mismatched    0',
  '  Normals unset (0,0,0) 0',
  '  Attribute bytes set   0',
  '',
  'Verdict',
  '  Print-ready           yes — closed, manifold, consistently wound, normals outward',
  '',
].join('\n');

const TETRA_REPORT_FORCED_ASCII = TETRA_REPORT.replace(
  'ASCII STL (text, auto-detected)',
  'ASCII STL (text)',
);

function cubeBinary(): Uint8Array {
  const p = [
    [0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0],
    [0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1],
  ];
  const quads = [
    [0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4],
    [2, 3, 7, 6], [0, 4, 7, 3], [1, 2, 6, 5],
  ];
  const tris: number[][][] = [];
  for (const q of quads) {
    tris.push([p[q[0]], p[q[1]], p[q[2]]]);
    tris.push([p[q[0]], p[q[2]], p[q[3]]]);
  }
  const bytes = new Uint8Array(84 + tris.length * 50);
  const view = new DataView(bytes.buffer);
  new TextEncoder().encode('solid binary cube').forEach((b, i) => (bytes[i] = b));
  view.setUint32(80, tris.length, true);
  let off = 84;
  for (const tri of tris) {
    const normal = unit(cross(sub(tri[1], tri[0]), sub(tri[2], tri[0])));
    for (const n of normal) { view.setFloat32(off, n, true); off += 4; }
    for (const v of tri) for (const n of v) { view.setFloat32(off, n, true); off += 4; }
    view.setUint16(off, 0, true); off += 2;
  }
  return bytes;
}

function sub(a: number[], b: number[]) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]; }
function cross(a: number[], b: number[]) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}
function unit(a: number[]) {
  const n = Math.hypot(a[0], a[1], a[2]);
  return n === 0 ? [0, 0, 0] : [a[0] / n, a[1] / n, a[2] / n];
}
function b64(bytes: Uint8Array) {
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary);
}
function hex(bytes: Uint8Array) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

async function runWasm(page: any, stl: string, inputFormat = 'auto', output = 'report', units = 'mm', scale = '1', density = '0', weldTolerance = '0.000001') {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/stl-inspector/gizza_ai_stl_inspector_web.js');
    await mod.default('/tools/stl-inspector/gizza_ai_stl_inspector_web_bg.wasm');
    return mod.run(args.stl, args.inputFormat, args.output, args.units, args.scale, args.density, args.weldTolerance);
  }, { stl, inputFormat, output, units, scale, density, weldTolerance });
}

test('stl-inspector page reports an ASCII tetrahedron exactly', async ({ page }) => {
  await page.goto('/tools/stl-inspector/');
  await page.fill('#in-stl', ASCII_TETRA);
  await expect(page.locator('#tool-output')).toContainText('Print-ready           yes', { timeout: 15_000 });
  expect(await page.locator('#tool-output').textContent()).toBe(TETRA_REPORT);
});

test('stl-inspector deep link pre-fills JSON output and density', async ({ page }) => {
  const params = new URLSearchParams({ stl: ASCII_TETRA, output: 'json', density: '1.24' });
  await page.goto(`/tools/stl-inspector/?${params.toString()}`);
  await expect(page.locator('#in-stl')).toHaveValue(ASCII_TETRA, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-density')).toHaveValue('1.24');
  await expect(page.locator('#tool-output')).toContainText('"weight_grams": 0.206667', { timeout: 15_000 });
  const report = JSON.parse((await page.locator('#tool-output').textContent())!);
  expect(report.print_ready).toBe(true);
  expect(report.triangles).toBe(4);
  expect(report.weight_grams).toBe(0.206667);
});

test('stl-inspector page reports an open mesh verdict', async ({ page }) => {
  await page.goto('/tools/stl-inspector/');
  await page.fill('#in-stl', SINGLE_TRIANGLE);
  await expect(page.locator('#tool-output')).toContainText('Print-ready           no — 3 open (boundary) edges', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Watertight            no');
});

test('stl-inspector example chips prefill and run', async ({ page }) => {
  await page.goto('/tools/stl-inspector/');
  const chips = page.locator('.tool-example-chip');
  await expect(chips).toHaveCount(4);
  await chips.nth(1).click();
  await expect(page.locator('#in-stl')).toHaveValue(SINGLE_TRIANGLE);
  await expect(page.locator('#tool-output')).toContainText('Triangles             1', { timeout: 15_000 });
});

test('stl-inspector wasm covers advertised enum and numeric option values', async ({ page }) => {
  await page.goto('/tools/stl-inspector/');
  await page.waitForSelector('#in-stl');
  const cube = cubeBinary();

  expect(await runWasm(page, ASCII_TETRA, 'ascii')).toBe(TETRA_REPORT_FORCED_ASCII);
  expect(await runWasm(page, b64(cube), 'base64')).toContain('Encoding              binary STL (base64)');
  expect(await runWasm(page, hex(cube), 'hex')).toContain('Encoding              binary STL (hex)');
  await expect(runWasm(page, ASCII_TETRA, 'obj')).rejects.toThrow(/unknown input_format/);

  const json = JSON.parse(await runWasm(page, ASCII_TETRA, 'auto', 'json'));
  expect(json.triangles).toBe(4);
  await expect(runWasm(page, ASCII_TETRA, 'auto', 'yaml')).rejects.toThrow(/unknown output/);

  expect(await runWasm(page, ASCII_TETRA, 'auto', 'report', 'cm')).toContain('Bounding box          10 x 10 x 10 cm');
  expect(await runWasm(page, ASCII_TETRA, 'auto', 'report', 'in')).toContain('Bounding box          10 x 10 x 10 in');
  await expect(runWasm(page, ASCII_TETRA, 'auto', 'report', 'ft')).rejects.toThrow(/unknown units/);

  expect(await runWasm(page, ASCII_TETRA, 'auto', 'report', 'mm', '2')).toContain('Bounding box          20 x 20 x 20 mm');
  expect(await runWasm(page, ASCII_TETRA, 'auto', 'report', 'mm', '1', '1.24')).toContain('Estimated weight      0.206667 g at 1.24 g/cm³');
  expect(await runWasm(page, SINGLE_TRIANGLE, 'auto', 'report', 'mm', '1', '0', '100')).toContain('Distinct vertices     1');
  await expect(runWasm(page, ASCII_TETRA, 'auto', 'report', 'mm', '-1')).rejects.toThrow(/scale must be a positive number/);
});

test('stl-inspector generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/stl-inspector/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool stl-inspector');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
