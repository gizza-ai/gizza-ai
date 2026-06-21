import { test, expect } from './fixtures';

// /tools/textrank-summarize/ extracts key sentences in-browser (pure wasm).
// text is a multiline <textarea>; sentences is a small field.
test('textrank-summarize page returns an extractive summary', async ({ page }) => {
  const text =
    'Cats are popular pets enjoyed worldwide. Cats purr and nap a lot. ' +
    'The stock market dropped sharply today amid fears. ' +
    'Investors sold stocks as the market fell. ' +
    'Cats remain a favorite companion animal in many homes.';

  await page.goto('/tools/textrank-summarize/');
  await page.fill('#in-text', text);
  await page.fill('#in-sentences', '2');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const summary = (await out.textContent())!.trim();
  // The summary is verbatim sentences from the source.
  const sentenceCount = (summary.match(/[.!?]/g) || []).length;
  expect(sentenceCount).toBeLessThanOrEqual(2);
  expect(sentenceCount).toBeGreaterThanOrEqual(1);
  // Each returned sentence must appear in the source text.
  for (const piece of summary.split(/(?<=[.!?])\s+/)) {
    const p = piece.trim();
    if (p) expect(text).toContain(p);
  }
});
