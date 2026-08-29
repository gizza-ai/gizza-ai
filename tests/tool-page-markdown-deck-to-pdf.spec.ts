import { test, expect } from './fixtures';

async function decodePdf(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl && dl.href;
    if (!href || !href.startsWith('data:application/pdf;base64,')) {
      return { error: 'no PDF download href: ' + href };
    }
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const latin1 = new TextDecoder('latin1').decode(buf);
    return {
      magicOk: buf[0] === 0x25 && buf[1] === 0x50 && buf[2] === 0x44 && buf[3] === 0x46 && buf[4] === 0x2d,
      len: buf.length,
      download: dl ? dl.getAttribute('download') : null,
      pageObjects: (latin1.match(/\/Type\s*\/Page\b/g) || []).length,
      hasDarkFill: latin1.includes('0.09 0.1 0.12 rg'),
    };
  });
}

test('markdown-deck-to-pdf page — H1 deck produces a real PDF download', async ({ page }) => {
  await page.goto('/tools/markdown-deck-to-pdf/');
  await page.fill('#in-title', 'Q3 Review');
  await page.fill('#in-markdown', '# Quarterly Review\n\n- Revenue up 24%\n- Two new markets\n\n# Next Steps\n\n- Hire 3 engineers');
  await page.selectOption('#in-split_level', 'h1');
  await page.selectOption('#in-slide_size', '16:9');
  await page.selectOption('#in-theme', 'light');
  await page.fill('#in-font_size', '20');
  await expect(page.locator('#tool-output')).toContainText('PDF deck ready', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'deck.pdf');

  const pdf = await decodePdf(page);
  expect(pdf.error).toBeUndefined();
  expect(pdf.magicOk).toBe(true);
  expect(pdf.download).toBe('deck.pdf');
  expect(pdf.len).toBeGreaterThan(2000);
  expect(pdf.pageObjects).toBe(3); // title slide + two H1 slides
});

test('markdown-deck-to-pdf page — deep-link chooses H2 split, dark 4:3 and no chrome', async ({ page }) => {
  const markdown = '# Launch Plan\n\n## Problem\n\n- Onboarding churn\n\n## Solution\n\n- Guided setup';
  await page.goto(
    `/tools/markdown-deck-to-pdf/?markdown=${encodeURIComponent(markdown)}&split_level=h2&slide_size=${encodeURIComponent('4:3')}&theme=dark&font_size=8&page_numbers=false&outline=false`,
  );
  await expect(page.locator('#tool-output')).toContainText('PDF deck ready', { timeout: 15000 });

  const pdf = await decodePdf(page);
  expect(pdf.error).toBeUndefined();
  expect(pdf.magicOk).toBe(true);
  expect(pdf.pageObjects).toBe(2);
  expect(pdf.hasDarkFill).toBe(true);
});

test('markdown-deck-to-pdf page — empty input is idle with no download', async ({ page }) => {
  await page.goto('/tools/markdown-deck-to-pdf/');
  await expect(page.locator('#tool-output')).toContainText('Paste a Markdown deck above', { timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeHidden();
  await expect(page.locator('#tool-output')).not.toHaveClass(/error/);
});
