import { test, expect } from './fixtures';

type ZipProbe = {
  error?: string;
  magicOk?: boolean;
  len?: number;
  download?: string | null;
  names?: string[];
  indexCsv?: string;
  firstSvg?: string;
};

/** Read the download anchor's data: URL and walk the ZIP local-file headers. */
async function decodeZip(page: import('@playwright/test').Page): Promise<ZipProbe> {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:application/zip;base64,')) {
      return { error: 'no ZIP href: ' + String(href).slice(0, 60) };
    }
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

test('barcode-batch page — CSV names produce a real Code 128 SVG ZIP with an index', async ({ page }) => {
  await page.goto('/tools/barcode-batch/');
  await page.selectOption('#in-format', 'svg');
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-columns', 'value-name');
  await page.fill('#in-data', 'SKU-1001,widget\nSKU-1002,gadget');
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const zip = await decodeZip(page);
  expect(zip.error).toBeUndefined();
  expect(zip.magicOk).toBe(true);
  expect(zip.download).toBe('barcode-batch.zip');
  expect(zip.names).toEqual(['widget.svg', 'gadget.svg', 'index.csv']);
  expect(zip.indexCsv).toContain('widget.svg,SKU-1001,Code 128,ok');
  expect(zip.indexCsv).toContain('gadget.svg,SKU-1002,Code 128,ok');
  // Real symbol, not an empty canvas: merged bar rects plus the printed value.
  expect(zip.firstSvg).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
  expect(zip.firstSvg).toContain('fill="#000000"');
  expect(zip.firstSvg).toContain('>SKU-1001</text>');
  expect((zip.firstSvg!.match(/<rect /g) || []).length).toBeGreaterThan(20);
});

test('barcode-batch page — deep-link computes UPC-A check digits and honours a non-default checkbox', async ({ page }) => {
  // ?param= deep-link: symbology + a NON-default (unchecked) include_index.
  await page.goto(
    '/tools/barcode-batch/?data=03600029145%0A01234565000&symbology=upca&format=svg&include_index=false&name_prefix=upc',
  );
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });

  const zip = await decodeZip(page);
  expect(zip.error).toBeUndefined();
  expect(zip.names).toEqual(['upc-001.svg', 'upc-002.svg']);
  expect(zip.indexCsv).toBe('');
  // 03600029145 is 11 digits; the tool must print the computed 12th (check) digit.
  expect(zip.firstSvg).toContain('>036000291452</text>');
});

test('barcode-batch page — advertised symbologies, colours and text toggle all render', async ({ page }) => {
  await page.goto('/tools/barcode-batch/');
  await page.selectOption('#in-format', 'svg');
  await page.selectOption('#in-symbology', 'ean13');
  await page.fill('#in-fg_color', '#f00'); // short hex must resolve, not fall through as text
  await page.fill('#in-bg_color', 'transparent');
  await page.uncheck('#in-show_text');
  await page.fill('#in-data', '5901234123457');
  await expect(page.locator('#tool-output')).toContainText('ZIP ready', { timeout: 15000 });

  const zip = await decodeZip(page);
  expect(zip.error).toBeUndefined();
  expect(zip.firstSvg).toContain('fill="#ff0000"');
  // transparent background paints no backdrop rect, and the HRI is suppressed.
  expect(zip.firstSvg).not.toContain('<rect width=');
  expect(zip.firstSvg).not.toContain('<text');
  // EAN-13 is a fixed 95-module symbol: 95 + 2x10 quiet, at 2px = 230px wide.
  expect(zip.firstSvg).toContain('width="230"');

  // A row that cannot be encoded is reported, not silently dropped.
  await page.selectOption('#in-symbology', 'ean13');
  await page.fill('#in-data', '5901234123457\nnot-a-number');
  await expect(page.locator('#tool-output')).toContainText('1 row error(s)', { timeout: 15000 });
});

test('barcode-batch page — sheet output downloads a real multi-label PDF', async ({ page }) => {
  await page.goto('/tools/barcode-batch/?output=sheet&sheet_preset=avery-5163&data=ASSET-0001%0AASSET-0002%0AASSET-0003');
  await expect(page.locator('#tool-output')).toContainText('PDF label sheet ready', { timeout: 15000 });

  const pdf = await page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:application/pdf;base64,')) {
      return { error: 'no PDF href: ' + String(href).slice(0, 60) };
    }
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const latin1 = new TextDecoder('latin1');
    const text = latin1.decode(buf);
    // Resolve every object through the xref table, the way a reader does.
    const sx = parseInt(text.slice(text.lastIndexOf('startxref\n') + 10).split('\n')[0], 10);
    const hdrEnd = text.indexOf('\n', sx + 5);
    const count = parseInt(text.slice(sx + 5, hdrEnd).split(/\s+/)[1], 10);
    const entries = text.slice(hdrEnd + 1);
    const badOffsets: number[] = [];
    for (let id = 1; id < count; id++) {
      const off = parseInt(entries.slice(id * 20, id * 20 + 10), 10);
      if (!text.slice(off).startsWith(`${id} 0 obj`)) badOffsets.push(id);
    }
    return {
      download: dl ? dl.getAttribute('download') : null,
      header: text.slice(0, 8),
      endsWithEof: text.endsWith('%%EOF\n'),
      xrefStartsTable: text.slice(sx, sx + 5) === 'xref\n',
      badOffsets,
      pageCount: (text.match(/\/Type \/Page \/Parent/g) || []).length,
      declaredCount: (text.match(/\/Count (\d+)/) || [])[1],
      mediaBox: (text.match(/\/MediaBox \[([^\]]+)\]/) || [])[1],
      len: buf.length,
    };
  });

  expect(pdf.error).toBeUndefined();
  expect(pdf.download).toBe('barcode-sheet.pdf');
  expect(pdf.header).toBe('%PDF-1.4');
  expect(pdf.endsWithEof).toBe(true);
  expect(pdf.xrefStartsTable).toBe(true);
  expect(pdf.badOffsets).toEqual([]);
  // 3 labels fit on one Avery 5163 sheet (10 up).
  expect(pdf.pageCount).toBe(1);
  expect(pdf.declaredCount).toBe('1');
  // US Letter at 72 pt/in.
  expect(pdf.mediaBox).toBe('0 0 612 792');
  expect(pdf.len).toBeGreaterThan(500);
});
