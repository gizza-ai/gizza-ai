import { test, expect } from './fixtures';

// csv-to-xlsx produces a BINARY .xlsx download, so "real output" means decoding
// the workbook the page offers and asserting its actual contents. The helper
// runs in the browser: it fetches the #tool-output-download data: URL, checks
// the ZIP magic, and inflates individual OOXML parts with DecompressionStream
// so we can assert cell text (shared strings) and native numeric cells.
async function decodeWorkbook(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:')) return { error: 'no download href: ' + href };
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const latin1 = new TextDecoder('latin1');
    const raw = latin1.decode(buf); // local-header filenames are stored uncompressed
    const magicOk = buf[0] === 0x50 && buf[1] === 0x4b && buf[2] === 0x03 && buf[3] === 0x04;

    async function entry(want: string): Promise<string | null> {
      let i = 0;
      while (i + 30 <= buf.length && dv.getUint32(i, true) === 0x04034b50) {
        const method = dv.getUint16(i + 8, true);
        const compSize = dv.getUint32(i + 18, true);
        const nameLen = dv.getUint16(i + 26, true);
        const extraLen = dv.getUint16(i + 28, true);
        const nameStart = i + 30;
        const name = latin1.decode(buf.subarray(nameStart, nameStart + nameLen));
        const dataStart = nameStart + nameLen + extraLen;
        const comp = buf.subarray(dataStart, dataStart + compSize);
        if (name === want) {
          if (method === 0) return new TextDecoder('utf-8').decode(comp);
          const stream = new Response(comp).body!.pipeThrough(new DecompressionStream('deflate-raw'));
          return new TextDecoder('utf-8').decode(await new Response(stream).arrayBuffer());
        }
        i = dataStart + compSize;
      }
      return null;
    }

    return {
      magicOk,
      len: buf.length,
      download: dl ? dl.getAttribute('download') : null,
      hasWorksheet: raw.includes('xl/worksheets/sheet1.xml'),
      hasContentTypes: raw.includes('[Content_Types].xml'),
      sharedStrings: await entry('xl/sharedStrings.xml'),
      sheet1: await entry('xl/worksheets/sheet1.xml'),
    };
  });
}

test('csv-to-xlsx page — CSV with typed columns produces a valid workbook', async ({ page }) => {
  await page.goto('/tools/csv-to-xlsx/');
  await page.fill('#in-sheet_name', 'People');
  await page.fill('#in-data', 'name,age,active\nAlice,30,true\nBob,25,false');
  await expect(page.locator('#tool-output')).toContainText('Workbook ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const wb = await decodeWorkbook(page);
  expect(wb.error).toBeUndefined();
  expect(wb.magicOk).toBe(true); // real ZIP/OOXML, not a renamed CSV
  expect(wb.hasWorksheet).toBe(true);
  expect(wb.hasContentTypes).toBe(true);
  expect(wb.download).toBe('People.xlsx'); // sheet name → download filename
  // Header + string cells land in the shared-string table.
  expect(wb.sharedStrings).toContain('name');
  expect(wb.sharedStrings).toContain('Alice');
  expect(wb.sharedStrings).toContain('Bob');
  expect(wb.sharedStrings).toContain('active');
  // age 30 is a NATIVE number cell (inline in the sheet), not shared text.
  expect(wb.sheet1).toContain('<v>30</v>');
  expect(wb.sharedStrings).not.toContain('>30<'); // 30 is not a string value
});

test('csv-to-xlsx page — JSON deep-link (?data=…&input_format=json)', async ({ page }) => {
  const json = '[{"product":"Widget","qty":12,"price":9.99},{"product":"Gadget","qty":3,"price":19.5}]';
  await page.goto(
    `/tools/csv-to-xlsx/?data=${encodeURIComponent(json)}&input_format=json&sheet_name=${encodeURIComponent('Sales')}`,
  );
  await expect(page.locator('#tool-output')).toContainText('Workbook ready', { timeout: 15000 });

  const wb = await decodeWorkbook(page);
  expect(wb.error).toBeUndefined();
  expect(wb.magicOk).toBe(true);
  expect(wb.download).toBe('Sales.xlsx');
  // JSON object keys become the header columns; string values are shared strings.
  expect(wb.sharedStrings).toContain('product');
  expect(wb.sharedStrings).toContain('Widget');
  expect(wb.sharedStrings).toContain('Gadget');
  // qty 12 is a native number cell.
  expect(wb.sheet1).toContain('<v>12</v>');
});

test('csv-to-xlsx page — detect-types OFF writes numbers as text (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/csv-to-xlsx/');
  await page.fill('#in-data', 'age\n30');
  // Default: 30 is a number → NOT in shared strings, present as <v>30</v>.
  await expect(page.locator('#tool-output')).toContainText('Workbook ready', { timeout: 15000 });
  let wb = await decodeWorkbook(page);
  expect(wb.sheet1).toContain('<v>30</v>');
  expect(wb.sharedStrings || '').not.toContain('>30<');

  // Toggle the non-default state: 30 must now be stored as TEXT.
  await page.uncheck('#in-detect_types');
  await expect(page.locator('#tool-output-download')).toBeVisible();
  wb = await decodeWorkbook(page);
  expect(wb.error).toBeUndefined();
  expect(wb.sharedStrings).toContain('30'); // 30 is now a shared string (text)
});

test('csv-to-xlsx page — empty input shows a neutral idle prompt, no download', async ({ page }) => {
  await page.goto('/tools/csv-to-xlsx/');
  await expect(page.locator('#tool-output')).toContainText('Paste some table data', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeHidden();
  await expect(page.locator('#tool-output')).not.toHaveClass(/error/);
});
