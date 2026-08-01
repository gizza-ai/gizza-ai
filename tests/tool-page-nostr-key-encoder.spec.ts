import { test, expect } from './fixtures';

const HEX = '7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e';
const NPUB_CORRECT = 'npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg';

test('nostr-key-encoder page encodes hex to npub', async ({ page }) => {
  await page.goto('/tools/nostr-key-encoder/');
  await page.fill('#in-input', HEX);
  await page.selectOption('#in-mode', 'encode');
  await page.selectOption('#in-type', 'npub');

  await expect(page.locator('#tool-output')).toHaveText(NPUB_CORRECT, { timeout: 15000 });
});

test('nostr-key-encoder honors deep-link decode params', async ({ page }) => {
  await page.goto('/tools/nostr-key-encoder/?input=' + encodeURIComponent(NPUB_CORRECT) + '&mode=decode&type=npub');
  await expect(page.locator('#tool-output')).toHaveText(HEX, { timeout: 15000 });
});
