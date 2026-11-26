import { expect, type Page } from '@playwright/test';

// Open a tool page at `url` (query params prefill the fields), upload
// `fixture` through `fileInput`, and return the result's MIME type and byte
// size. The result URL may be data: or blob:; fetching it in the page reads
// either. Used by the quality-direction specs (a higher quality must give a
// larger file for every lossy encoder).
export async function outputSize(
  page: Page,
  url: string,
  fileInput: string,
  fixture: string,
  prefilled: Record<string, string>,
): Promise<{ mime: string; bytes: number }> {
  await page.goto(url);
  await page.waitForSelector(fileInput);
  for (const [id, value] of Object.entries(prefilled)) {
    await expect(page.locator(id)).toHaveValue(value, { timeout: 15_000 });
  }
  await page.setInputFiles(fileInput, fixture);
  // ffmpeg loads from the CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = (await media.getAttribute('src'))!;
  return page.evaluate(async (u) => {
    const blob = await (await fetch(u)).blob();
    return { mime: blob.type, bytes: blob.size };
  }, src);
}
