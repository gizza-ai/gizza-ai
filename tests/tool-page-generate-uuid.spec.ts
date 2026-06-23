import { test, expect } from './fixtures';

// /tools/generate-uuid/ generates RFC 4122 / RFC 9562 UUIDs in-browser.
const UUID_RE = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/;

test('generate-uuid page produces a v4 UUID on load', async ({ page }) => {
  await page.goto('/tools/generate-uuid/');
  await expect(page.locator('#tool-output')).toContainText(UUID_RE, { timeout: 15000 });
  const out = (await page.locator('#tool-output').textContent()) ?? '';
  const m = out.match(UUID_RE);
  expect(m).not.toBeNull();
  // v4 has version nibble '4' and a variant nibble in {8,9,a,b}.
  expect(m![0][14]).toBe('4');
  expect('89ab').toContain(m![0][19]);
});

test('generate-uuid page makes a bulk batch', async ({ page }) => {
  await page.goto('/tools/generate-uuid/');
  await page.fill('#in-count', '5');
  await expect
    .poll(async () => {
      const out = (await page.locator('#tool-output').textContent()) ?? '';
      return (out.match(new RegExp(UUID_RE, 'g')) ?? []).length;
    }, { timeout: 15000 })
    .toBe(5);
});

test('generate-uuid v5 namespace is deterministic via deep-link', async ({ page }) => {
  await page.goto('/tools/generate-uuid/?version=5&namespace=dns&name=www.example.com');
  // uuid5(NAMESPACE_DNS, "www.example.com") is a fixed RFC-derived value.
  await expect(page.locator('#tool-output')).toContainText(
    '2ed6657d-e927-568b-95e1-2665a8aea6a2',
    { timeout: 15000 },
  );
});
