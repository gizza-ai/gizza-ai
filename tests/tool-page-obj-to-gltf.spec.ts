import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const triangleObj = 'v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3';

test('obj-to-gltf converts a pasted OBJ triangle to embedded glTF JSON', async ({ page }) => {
  await page.goto('/tools/obj-to-gltf/');
  await setField(page, '#in-obj', triangleObj);
  await page.selectOption('#in-to', 'gltf');
  await setField(page, '#in-name', 'triangle');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"generator": "gizza-ai/obj-to-gltf"', { timeout: 15_000 });
  await expect(out).toContainText('"name": "triangle"');
  await expect(out).toContainText('"attributes": { "POSITION": 0, "NORMAL": 1 }');
  await expect(out).toContainText('"byteLength": 84');
  await expect(out).toContainText('data:application/octet-stream;base64,');
});

test('obj-to-gltf honors deep-linked GLB, material and checkbox options', async ({ page }) => {
  const params = new URLSearchParams({
    obj: 'v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nusemtl red\nf 1 2 3 4',
    mtl: 'newmtl red\nKd 1 0 0\nNs 32',
    to: 'glb',
    up_axis: 'y',
    scale: '1',
    normals: 'flat',
    name: 'red_panel',
    unlit: 'true',
    double_sided: 'true',
  });
  await page.goto(`/tools/obj-to-gltf/?${params.toString()}`);

  await expect(page.locator('#in-to')).toHaveValue('glb');
  await expect(page.locator('#in-normals')).toHaveValue('flat');
  await expect(page.locator('#in-unlit')).toBeChecked();
  await expect(page.locator('#in-double_sided')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('data:model/gltf-binary;base64,Z2xURgIA', { timeout: 15_000 });
});

test('obj-to-gltf covers enum choices, scale boundary and error surface', async ({ page }) => {
  await page.goto('/tools/obj-to-gltf/');
  await setField(page, '#in-obj', 'v 0 0 0\nv 1000 0 0\nv 0 0 1000\nf 1 2 3');
  await page.selectOption('#in-to', 'gltf');
  await page.selectOption('#in-up_axis', 'z');
  await setField(page, '#in-scale', '0.001');
  await page.selectOption('#in-normals', 'none');
  await setField(page, '#in-name', 'z_up_triangle');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "z_up_triangle"', { timeout: 15_000 });
  await expect(out).toContainText('"min": [0,0,0], "max": [1,1,0]');
  await expect(out).not.toContainText('"NORMAL"');

  await setField(page, '#in-scale', '0');
  await expect(page.locator('#tool-output')).toContainText('scale must be a non-zero finite number', { timeout: 15_000 });
});
