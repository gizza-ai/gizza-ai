import { test, expect } from './fixtures';

async function decodeZip(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:application/zip;base64,')) return { error: 'no ZIP href: ' + href };
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const latin1 = new TextDecoder('latin1');
    const names: string[] = [];

    async function inflate(comp: Uint8Array, method: number): Promise<string> {
      if (method === 0) return new TextDecoder().decode(comp);
      const stream = new Response(comp).body!.pipeThrough(new DecompressionStream('deflate-raw'));
      return new TextDecoder().decode(await new Response(stream).arrayBuffer());
    }

    let i = 0;
    let indexCsv = '';
    let firstSvg = '';
    while (i + 30 <= buf.length && dv.getUint32(i, true) === 0x04034b50) {
      const method = dv.getUint16(i + 8, true);
      const compSize = dv.getUint32(i + 18, true);
      const nameLen = dv.getUint16(i + 26, true);
      const extraLen = dv.getUint16(i + 28, true);
      const nameStart = i + 30;
      const name = latin1.decode(buf.subarray(nameStart, nameStart + nameLen));
      const dataStart = nameStart + nameLen + extraLen;
      const comp = buf.subarray(dataStart, dataStart + compSize);
      names.push(name);
      if (name === 'index.csv') indexCsv = await inflate(comp, method);
      if (name.endsWith('.svg') && !firstSvg) firstSvg = await inflate(comp, method);
      i = dataStart + compSize;
    }
    return {
      magicOk: buf[0] === 0x50 && buf[1] === 0x4b && buf[2] === 0x03 && buf[3] === 0x04,
      len: buf.length,
      download: dl ? dl.getAttribute('download') : null,
      names,
      indexCsv,
      firstSvg,
    };
  });
}

test('qr-batch page — CSV names produce real SVG ZIP with index', async ({ page }) => {
  await page.goto('/tools/qr-batch/');
  await page.selectOption('#in-format', 'svg');
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-columns', 'name-value');
  await page.fill('#in-data', 'homepage,https://example.com\nsupport,mailto:support@example.com');
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const zip = await decodeZip(page);
  expect(zip.error).toBeUndefined();
  expect(zip.magicOk).toBe(true);
  expect(zip.download).toBe('qr-batch.zip');
  expect(zip.names).toEqual(['homepage.svg', 'support.svg', 'index.csv']);
  expect(zip.indexCsv).toContain('homepage.svg,https://example.com,ok');
  expect(zip.indexCsv).toContain('support.svg,mailto:support@example.com,ok');
  expect(zip.firstSvg).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
  expect(zip.firstSvg).toContain('fill="#000000"');
});

test('qr-batch page — deep-link and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/qr-batch/?data=sku-1%0Asku-2&format=svg&include_index=false&name_prefix=asset');
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });
  const zip = await decodeZip(page);
  expect(zip.error).toBeUndefined();
  expect(zip.names).toEqual(['asset-001.svg', 'asset-002.svg']);
  expect(zip.indexCsv).toBe('');
});

test('qr-batch page — advertised colour and error correction values render', async ({ page }) => {
  await page.goto('/tools/qr-batch/');
  await page.selectOption('#in-format', 'svg');
  await page.selectOption('#in-error_correction', 'H');
  await page.fill('#in-fg_color', '#f00');
  await page.fill('#in-bg_color', 'transparent');
  await page.fill('#in-data', 'red-code');
  await expect(page.locator('#tool-output-download')).toBeVisible({ timeout: 15000 });
  const zip = await decodeZip(page);
  expect(zip.firstSvg).toContain('fill="#ff0000"');
  expect(zip.firstSvg).not.toContain('<rect');
});

test('qr-batch page — cap boundary and one-over error', async ({ page }) => {
  const rows = Array.from({ length: 500 }, (_, i) => `row-${i + 1}`).join('\n');
  await page.goto('/tools/qr-batch/');
  await page.selectOption('#in-format', 'svg');
  await page.locator('#in-data').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, rows);
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });
  const zip = await decodeZip(page);
  expect(zip.names).toHaveLength(501); // 500 SVGs + index.csv

  await page.locator('#in-data').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, rows + '\nrow-501');
  await expect(page.locator('#tool-output')).toContainText('over the 500-row batch cap', { timeout: 15000 });
});
