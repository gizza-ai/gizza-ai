import { test, expect } from './fixtures';

const triangleObj = `# triangle
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vt 0.0 0.0
vn 0.0 0.0 1.0
f 1/1/1 2/1/1 3/1/1
`;

const coloredObj = `v 0 0 0 1.0 0.0 0.0
v 1 0 0
`;

const multiObjectObj = `o chassis
v 0 0 0
v 1 0 0
o wheels
v 2 0 0
v 3 0 0
f 1 2 3
`;

test('obj-vertices-to-csv page extracts default x y z CSV', async ({ page }) => {
  await page.goto('/tools/obj-vertices-to-csv/');
  await page.fill('#in-obj', triangleObj);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('x,y,z', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`x,y,z
0.0,0.0,0.0
1.0,0.0,0.0
0.0,1.0,0.0`);
});

test('obj-vertices-to-csv page deep-link filters object rows and preserves source index', async ({ page }) => {
  const qs =
    '?obj=' +
    encodeURIComponent(multiObjectObj) +
    '&columns=object' +
    '&color=auto' +
    '&up_axis=keep' +
    '&scale=1' +
    '&precision=-1' +
    '&dedupe=none' +
    '&objects=wheels' +
    '&delimiter=comma' +
    '&header=true';
  await page.goto('/tools/obj-vertices-to-csv/' + qs);

  await expect(page.locator('#in-columns')).toHaveValue('object', { timeout: 15_000 });
  await expect(page.locator('#in-objects')).toHaveValue('wheels');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,object,group,x,y,z', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`index,object,group,x,y,z
3,wheels,,2,0,0
4,wheels,,3,0,0`);
});

test('obj-vertices-to-csv page emits plain XYZ point-cloud text', async ({ page }) => {
  await page.goto('/tools/obj-vertices-to-csv/');
  await page.fill('#in-obj', 'v 0.12 0.45 1.98\nv 0.13 0.46 1.97\nv 0.13 0.46 1.97\n');
  await page.selectOption('#in-delimiter', 'space');
  await page.locator('#in-header').uncheck();
  await page.fill('#in-precision', '3');
  await page.selectOption('#in-dedupe', 'all');

  await expect(page.locator('#tool-output')).toHaveText('0.120 0.450 1.980\n0.130 0.460 1.970', {
    timeout: 15_000,
  });
});

test('obj-vertices-to-csv page handles colour columns and axis scaling', async ({ page }) => {
  await page.goto('/tools/obj-vertices-to-csv/');
  await page.fill('#in-obj', coloredObj);
  await page.selectOption('#in-color', 'always');
  await page.selectOption('#in-up_axis', 'y-to-z');
  await page.fill('#in-scale', '1000');
  await page.fill('#in-precision', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('x,y,z,red,green,blue', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`x,y,z,red,green,blue
0,0,0,1.0,0.0,0.0
1000,0,0,,,`);
});
