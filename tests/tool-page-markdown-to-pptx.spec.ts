import { test, expect } from './fixtures';

// markdown-to-pptx returns a binary OOXML presentation as a data: URL. Decode
// the ZIP in-browser and inflate slide XML so the page test proves real output,
// not just that a Download button appeared.
async function decodePptx(page: import('@playwright/test').Page) {
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

    const slideNames = Array.from(raw.matchAll(/ppt\/slides\/slide\d+\.xml/g)).map((m) => m[0]);
    return {
      magicOk,
      len: buf.length,
      download: dl ? dl.getAttribute('download') : null,
      hasContentTypes: raw.includes('[Content_Types].xml'),
      hasPresentation: raw.includes('ppt/presentation.xml'),
      slideCount: new Set(slideNames).size,
      slide1: await entry('ppt/slides/slide1.xml'),
      slide2: await entry('ppt/slides/slide2.xml'),
      slide3: await entry('ppt/slides/slide3.xml'),
      presentation: await entry('ppt/presentation.xml'),
    };
  });
}

test('markdown-to-pptx page — H1 outline produces a valid editable PPTX', async ({ page }) => {
  await page.goto('/tools/markdown-to-pptx/');
  await page.fill('#in-title', 'Q3 Review');
  await page.fill('#in-markdown', '# Quarterly Review\n\n- Revenue up 24%\n- Two new markets\n\n# Next Steps\n\n- Hire 3 engineers');
  await page.selectOption('#in-split_level', 'h1');
  await expect(page.locator('#tool-output')).toContainText('Presentation ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const pptx = await decodePptx(page);
  expect(pptx.error).toBeUndefined();
  expect(pptx.magicOk).toBe(true);
  expect(pptx.hasContentTypes).toBe(true);
  expect(pptx.hasPresentation).toBe(true);
  expect(pptx.download).toBe('presentation.pptx');
  expect(pptx.slideCount).toBe(3); // title slide + two H1 slides
  expect(pptx.slide1).toContain('Q3 Review');
  expect(pptx.slide2).toContain('Quarterly Review');
  expect(pptx.slide2).toContain('Revenue up 24%');
  expect(pptx.slide3).toContain('Next Steps');
});

test('markdown-to-pptx page — deep-link params choose H2 split and dark 4:3 deck', async ({ page }) => {
  const markdown = '# Launch Plan\n\n## Problem\n\n- Onboarding churn\n\n## Solution\n\n- Guided setup';
  await page.goto(
    `/tools/markdown-to-pptx/?markdown=${encodeURIComponent(markdown)}&split_level=h2&theme=dark&aspect_ratio=${encodeURIComponent('4:3')}`,
  );
  await expect(page.locator('#tool-output')).toContainText('Presentation ready', { timeout: 15000 });

  const pptx = await decodePptx(page);
  expect(pptx.error).toBeUndefined();
  expect(pptx.magicOk).toBe(true);
  expect(pptx.slideCount).toBe(2);
  expect(pptx.slide1).toContain('Problem');
  expect(pptx.slide2).toContain('Solution');
  expect(pptx.presentation).toContain('screen4x3');
  expect(pptx.presentation).toContain('cx="9144000"');
});
