import { test, expect } from './fixtures';

// /tools/wireguard-config-builder/ assembles & validates a wg0.conf from
// pasted keys/addresses/endpoint (pure wasm). All inputs are <input> fields
// except format which is a <select> (conf | json). Query params match the API
// names and auto-run. Keys below are base64 of exactly 32 bytes (not secrets).
const PRIVATE_KEY = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';
const PEER_PUBLIC_KEY = 'MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=';

test('wireguard-config-builder page assembles a full-tunnel wg0.conf', async ({ page }) => {
  await page.goto('/tools/wireguard-config-builder/');
  await page.fill('#in-private_key', PRIVATE_KEY);
  await page.fill('#in-address', '10.0.0.2/32');
  await page.fill('#in-dns', '1.1.1.1, 8.8.8.8');
  await page.fill('#in-peer_public_key', PEER_PUBLIC_KEY);
  await page.fill('#in-allowed_ips', '0.0.0.0/0, ::/0');
  await page.fill('#in-endpoint', 'vpn.example.com:51820');
  await page.fill('#in-persistent_keepalive', '25');
  await page.selectOption('#in-format', 'conf');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('[Interface]', { timeout: 15000 });
  for (const line of [
    'PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=',
    'Address = 10.0.0.2/32',
    'DNS = 1.1.1.1, 8.8.8.8',
    '[Peer]',
    'PublicKey = MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=',
    'AllowedIPs = 0.0.0.0/0, ::/0',
    'Endpoint = vpn.example.com:51820',
    'PersistentKeepalive = 25',
  ]) {
    await expect(out).toContainText(line);
  }
});

test('wireguard-config-builder page prefills a split-tunnel from query params', async ({ page }) => {
  const params = new URLSearchParams({
    private_key: PRIVATE_KEY,
    address: '10.0.0.2/32',
    peer_public_key: PEER_PUBLIC_KEY,
    allowed_ips: '10.0.0.0/24',
    endpoint: 'vpn.example.com:51820',
    format: 'conf',
  });
  await page.goto(`/tools/wireguard-config-builder/?${params.toString()}`);
  await expect(page.locator('#in-allowed_ips')).toHaveValue('10.0.0.0/24');
  await expect(page.locator('#tool-output')).toContainText('AllowedIPs = 10.0.0.0/24', {
    timeout: 15000,
  });
});

test('wireguard-config-builder page reports an invalid key', async ({ page }) => {
  await page.goto('/tools/wireguard-config-builder/');
  await page.fill('#in-private_key', 'AAAA'); // valid base64 but only 3 bytes, not 32
  await page.fill('#in-address', '10.0.0.2/32');
  await page.fill('#in-peer_public_key', PEER_PUBLIC_KEY);
  await page.fill('#in-allowed_ips', '10.0.0.0/24');
  await expect(page.locator('#tool-output')).toContainText('not 32', { timeout: 15000 });
});
