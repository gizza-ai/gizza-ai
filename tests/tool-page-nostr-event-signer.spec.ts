import { test, expect } from './fixtures';

const NSEC = 'nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5';
const HEX_SECRET = '0000000000000000000000000000000000000000000000000000000000000003';

async function fillBase(page: import('@playwright/test').Page) {
  await page.fill('#in-nsec', NSEC);
  await page.fill('#in-content', 'hello from gizza');
  await page.fill('#in-kind', '1');
  await page.fill('#in-tags', 't=tools');
  await page.fill('#in-created_at', '1700000000');
}

test('nostr-event-signer signs a deterministic text note', async ({ page }) => {
  await page.goto('/tools/nostr-event-signer/');
  await fillBase(page);
  await page.uncheck('#in-pretty');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe(
      '{"id":"3e079f2b43df8d22a2622277668781f7aad7edabcd0ecf7ca9428a4652ecc918","pubkey":"7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e","created_at":1700000000,"kind":1,"tags":[["t","tools"]],"content":"hello from gizza","sig":"6ac80406093cad38820f95d772bc3a29324a8c3441785ca9e64f53af23b1e728a0028abb4cebbd949d44421249da8624e6160203aa9a757ee6c99658730c678e"}',
    );
});

test('nostr-event-signer emits relay EVENT frame from hex secret', async ({ page }) => {
  await page.goto('/tools/nostr-event-signer/');
  await page.fill('#in-nsec', HEX_SECRET);
  await page.fill('#in-content', 'publish me');
  await page.fill('#in-kind', '1');
  await page.fill('#in-created_at', '1700000000');
  await page.selectOption('#in-output', 'relay-message');
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('["EVENT",{"id":"5719a52ef9173c3d90a93c91aacb9898918e645bca73a11d1888e8c429b39496"', { timeout: 15000 });
  await expect(out).toContainText('"pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"');
  await expect(out).toContainText('"content":"publish me"');
});

test('nostr-event-signer report mode includes nip19 forms and verification', async ({ page }) => {
  await page.goto('/tools/nostr-event-signer/');
  await fillBase(page);
  await page.selectOption('#in-output', 'report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('id: 3e079f2b43df8d22a2622277668781f7aad7edabcd0ecf7ca9428a4652ecc918', { timeout: 15000 });
  await expect(out).toContainText('note: note18cre726rm7xj9gnzyfmkdpup774d0mdte58v7l9fg29yv5hveyvqx7ksdz');
  await expect(out).toContainText('npub: npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg');
  await expect(out).toContainText('signature check: valid');
});

test('nostr-event-signer deep-link template overrides fields', async ({ page }) => {
  const template = JSON.stringify({
    kind: 7,
    content: '+',
    tags: [['e', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef']],
    created_at: 1700000000,
  });
  await page.goto(
    '/tools/nostr-event-signer/?nsec=' +
      encodeURIComponent(NSEC) +
      '&content=ignored&template=' +
      encodeURIComponent(template) +
      '&output=event',
  );
  await expect(page.locator('#in-template')).toHaveValue(template, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"kind":7', { timeout: 15000 });
  await expect(out).toContainText('"content":"+"');
  await expect(out).toContainText('0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef');
});

test('nostr-event-signer validates secret-key input', async ({ page }) => {
  await page.goto('/tools/nostr-event-signer/');
  await page.fill('#in-nsec', 'npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqyf2h2');
  await page.fill('#in-content', 'x');
  await page.fill('#in-created_at', '1700000000');
  await expect(page.locator('#tool-output')).toContainText(
    'npub1 is a public identifier, not a private key',
    { timeout: 15000 },
  );
});
