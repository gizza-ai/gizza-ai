import { test, expect } from './fixtures';

// /tools/ip-log-anonymizer/ masks / salted-hashes / redacts every IPv4 & IPv6
// address in a log, in place (pure wasm). mode is a <select>; ipv4_octets /
// ipv6_groups / hash_length are number fields (canonical #in-<name>);
// skip_private is a checkbox. Output is the whole text with only addresses
// rewritten, so compare #tool-output textContent exactly.

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('mask mode (default) zeros the last octet, keeps the port and text', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', 'client=203.0.113.45:8080 GET');
  await expect(page.locator('#tool-output')).toContainText('203.0.113.0', { timeout: 15000 });
  expect(await outText(page)).toBe('client=203.0.113.0:8080 GET');
});

test('redact mode replaces each address with the placeholder token', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', 'a 192.0.2.1 b 198.51.100.7 c');
  await page.selectOption('#in-mode', 'redact');
  await page.fill('#in-replacement', '[IP]');
  await expect(page.locator('#tool-output')).toContainText('[IP]', { timeout: 15000 });
  expect(await outText(page)).toBe('a [IP] b [IP] c');
});

test('hash mode is salted, deterministic and truncated (same IP → same token)', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', '203.0.113.45 203.0.113.45');
  await page.selectOption('#in-mode', 'hash');
  await page.fill('#in-salt', 's3cret');
  await page.fill('#in-hash_length', '12');
  // wait for a "<12 hex> <12 hex>" shape to settle, then assert both tokens match.
  await expect
    .poll(async () => outText(page), { timeout: 15000 })
    .toMatch(/^[0-9a-f]{12} [0-9a-f]{12}$/);
  const out = await outText(page);
  const [a, b] = out.split(' ');
  expect(a).toHaveLength(12);
  expect(a).toBe(b); // same address + salt → identical pseudonym
  expect(out).not.toContain('203.0.113.45'); // the raw IP is gone
});

test('deep-link pre-fills and auto-runs; skip_private checkbox leaves internal IPs', async ({ page }) => {
  const text = encodeURIComponent('10.0.0.5 -> 203.0.113.9');
  await page.goto(`/tools/ip-log-anonymizer/?mode=mask&ipv4_octets=1&skip_private=true&text=${text}`);
  await expect(page.locator('#tool-output')).toContainText('203.0.113.0', { timeout: 15000 });
  // 10.0.0.5 is private → untouched; the public address is masked to /24.
  expect(await outText(page)).toBe('10.0.0.5 -> 203.0.113.0');
});

test('hash_length cap: 64 yields a full 64-hex token', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', '203.0.113.45');
  await page.selectOption('#in-mode', 'hash');
  await page.fill('#in-hash_length', '64');
  await expect.poll(async () => outText(page), { timeout: 15000 }).toMatch(/^[0-9a-f]{64}$/);
  expect(await outText(page)).not.toContain('203.0.113.45');
});

test('ipv4_octets cap: 4 masks the whole IPv4 address to 0.0.0.0', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', '203.0.113.45');
  await page.fill('#in-ipv4_octets', '4');
  await expect(page.locator('#tool-output')).toContainText('0.0.0.0', { timeout: 15000 });
  expect(await outText(page)).toBe('0.0.0.0');
});

test('ipv6_groups cap: 8 masks the whole IPv6 address to ::', async ({ page }) => {
  await page.goto('/tools/ip-log-anonymizer/');
  await page.fill('#in-text', '2001:db8:85a3::8a2e:370:7334');
  await page.fill('#in-ipv6_groups', '8');
  await expect.poll(async () => outText(page), { timeout: 15000 }).toBe('::');
});
