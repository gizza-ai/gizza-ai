import { test, expect } from './fixtures';

const EMPTY_SCENE = '{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}] }';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('gltf-glb-converter summarizes glTF to GLB conversion', async ({ page }) => {
  await page.goto('/tools/gltf-glb-converter/');
  await setField(page, '#in-model', EMPTY_SCENE);
  await page.selectOption('#in-input_format', 'gltf');
  await page.selectOption('#in-to', 'glb');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Direction         glTF JSON -> GLB', { timeout: 15_000 });
  await expect(out).toContainText('Scenes / nodes    1 / 0');
  await expect(out).toContainText('Output');
  await expect(out).toContainText('JSON chunk        52 B');
  await expect(out).toContainText('Byte-exact copy   yes');
});

test('gltf-glb-converter honors deep-linked base64 and GLB-to-glTF parameters', async ({ page }) => {
  const glbHex = '676c54460200000048000000340000004a534f4e7b226173736574223a7b2276657273696f6e223a22322e30227d2c227363656e6573223a5b7b226e6f646573223a5b5d7d5d7d20';
  const params = new URLSearchParams({
    model: glbHex,
    input_format: 'hex',
    to: 'gltf',
    output: 'file',
    images: 'auto',
    buffer_uri: 'scene.bin',
    pretty: 'false',
    output_encoding: 'base64',
  });
  await page.goto(`/tools/gltf-glb-converter/?${params.toString()}`);

  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-to')).toHaveValue('gltf');
  await expect(page.locator('#in-buffer_uri')).toHaveValue('scene.bin');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"asset":{"version":"2.0"}', { timeout: 15_000 });
  await expect(out).toContainText('"scenes":[{"nodes":[]}]}');
});

test('gltf-glb-converter covers enum choices, checkbox and hex output', async ({ page }) => {
  await page.goto('/tools/gltf-glb-converter/');
  await setField(page, '#in-model', EMPTY_SCENE);
  await page.selectOption('#in-input_format', 'auto');
  await page.selectOption('#in-to', 'glb');
  await page.selectOption('#in-output', 'file');
  await page.selectOption('#in-images', 'buffer');
  await page.uncheck('#in-pretty');
  await page.selectOption('#in-output_encoding', 'hex');

  await expect(page.locator('#tool-output')).toContainText('676c544602000000', { timeout: 15_000 });

  await page.selectOption('#in-output_encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('Z2xURgIAAABIAAAANAAAAEpT', { timeout: 15_000 });

  await setField(page, '#in-model', 'not a model');
  await page.selectOption('#in-input_format', 'gltf');
  await expect(page.locator('#tool-output')).toContainText("instead of '{'", { timeout: 15_000 });
});
