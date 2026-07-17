import { test, expect } from './fixtures';

async function decodeDocx(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:')) return { error: 'no download href: ' + href };
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const latin1 = new TextDecoder('latin1');
    const raw = latin1.decode(buf);
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
      hasContentTypes: raw.includes('[Content_Types].xml'),
      hasDocument: raw.includes('word/document.xml'),
      hasStyles: raw.includes('word/styles.xml'),
      document: await entry('word/document.xml'),
      styles: await entry('word/styles.xml'),
      core: await entry('docProps/core.xml'),
    };
  });
}

test('markdown-to-docx page — markdown becomes a valid editable DOCX', async ({ page }) => {
  await page.goto('/tools/markdown-to-docx/');
  await page.fill('#in-title', 'Project Brief');
  await page.fill('#in-markdown', '# Project brief\n\nThis is **ready** for review.\n\n- Scope\n- Timeline\n\n> Keep a copy.');
  await page.selectOption('#in-page_size', 'letter');
  await page.selectOption('#in-font_family', 'calibri');
  await page.fill('#in-font_size', '11');
  await expect(page.locator('#tool-output')).toContainText('Document ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const docx = await decodeDocx(page);
  expect(docx.error).toBeUndefined();
  expect(docx.magicOk).toBe(true);
  expect(docx.hasContentTypes).toBe(true);
  expect(docx.hasDocument).toBe(true);
  expect(docx.hasStyles).toBe(true);
  expect(docx.download).toBe('document.docx');
  expect(docx.document).toContain('Project brief');
  expect(docx.document).toContain('ready');
  expect(docx.document).toContain('Scope');
  expect(docx.document).toContain('Quote');
  expect(docx.core).toContain('Project Brief');
});

test('markdown-to-docx page — deep-link params choose A4, Times and page breaks', async ({ page }) => {
  const markdown = '# Summary\n\nFirst page.\n\n---\n\n# Appendix\n\nSecond page.';
  await page.goto(
    `/tools/markdown-to-docx/?markdown=${encodeURIComponent(markdown)}&title=${encodeURIComponent('Report')}&page_size=a4&font_family=times_new_roman&font_size=12&page_break=true`,
  );
  await expect(page.locator('#tool-output')).toContainText('Document ready', { timeout: 15000 });

  const docx = await decodeDocx(page);
  expect(docx.error).toBeUndefined();
  expect(docx.magicOk).toBe(true);
  expect(docx.document).toContain('Summary');
  expect(docx.document).toContain('Appendix');
  expect(docx.document).toContain('<w:br w:type="page"/>');
  expect(docx.document).toContain('w:w="11906"');
  expect(docx.styles).toContain('Times New Roman');
});
