import { test, expect } from './fixtures';

const ID = '1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW';
const SHARE = `https://drive.google.com/file/d/${ID}/view?usp=sharing`;

async function outputText(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15_000 });
  return ((await out.textContent()) ?? '').trim();
}

test('gdrive-link-converter turns a share link into an exact direct-download URL', async ({ page }) => {
  await page.goto('/tools/gdrive-link-converter/');
  await page.fill('#in-input', SHARE);
  await page.selectOption('#in-output', 'direct');

  await expect(page.locator('#tool-output')).toContainText(
    `https://drive.google.com/uc?export=download&id=${ID}`,
    { timeout: 15_000 },
  );
  expect(await outputText(page)).toBe(`https://drive.google.com/uc?export=download&id=${ID}`);
});

test('gdrive-link-converter exercises every output form with exact text', async ({ page }) => {
  await page.goto('/tools/gdrive-link-converter/');
  await page.fill('#in-input', SHARE);

  const cases: Array<[string, string]> = [
    ['direct_confirm', `https://drive.usercontent.google.com/download?id=${ID}&export=download&confirm=t`],
    ['view', `https://drive.google.com/uc?export=view&id=${ID}`],
    ['share', `https://drive.google.com/file/d/${ID}/view?usp=sharing`],
    ['preview', `https://drive.google.com/file/d/${ID}/preview`],
    ['id', ID],
  ];

  for (const [value, expected] of cases) {
    await page.selectOption('#in-output', value);
    await expect(page.locator('#tool-output')).toContainText(expected, { timeout: 15_000 });
    expect(await outputText(page)).toBe(expected);
  }
});

test('gdrive-link-converter builds thumbnail URLs with a custom size', async ({ page }) => {
  await page.goto('/tools/gdrive-link-converter/');
  await page.fill('#in-input', `https://drive.google.com/open?id=${ID}`);
  await page.selectOption('#in-output', 'thumbnail');
  await page.fill('#in-size', 'w320-h240');

  expect(await outputText(page)).toBe(`https://drive.google.com/thumbnail?id=${ID}&sz=w320-h240`);
});

test('gdrive-link-converter deep-links a batch conversion and non-default checkbox state', async ({ page }) => {
  const id2 = 'ABCDEFGHIJKL_MNOPQRSTUVWX';
  const input = `${SHARE}\n\nhttps://drive.google.com/open?id=${id2}`;
  const qs = new URLSearchParams({
    input,
    output: 'direct',
    per_line: 'true',
  });
  await page.goto(`/tools/gdrive-link-converter/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(input);
  await expect(page.locator('#in-per_line')).toBeChecked();

  expect(await outputText(page)).toBe(
    `https://drive.google.com/uc?export=download&id=${ID}\n\nhttps://drive.google.com/uc?export=download&id=${id2}`,
  );
});
