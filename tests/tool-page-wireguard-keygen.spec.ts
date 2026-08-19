import { test, expect } from './fixtures';

const tool = '/tools/wireguard-keygen/';
const keyRe = '[A-Za-z0-9+/]{43}=';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

function expectWireGuardKeys(out: string, { preshared = true, count = 1 } = {}): void {
  // Text/conf output repeats the same generated keys inside the sample config and
  // the peer fragment, so assert the generated-key shape appears at least once
  // per requested pair rather than exactly once.
  expect(out.match(new RegExp(`PrivateKey\\s+= ${keyRe}`, 'g'))?.length ?? 0).toBeGreaterThanOrEqual(count);
  expect(out.match(new RegExp(`PublicKey\\s+= ${keyRe}`, 'g'))?.length ?? 0).toBeGreaterThanOrEqual(count);
  if (preshared) {
    expect(out.match(new RegExp(`PresharedKey = ${keyRe}`, 'g'))?.length).toBeGreaterThanOrEqual(count);
  } else {
    expect(out).not.toContain('PresharedKey');
  }
}

test('wireguard-keygen page emits WireGuard-shaped keys and a config snippet', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-address', '10.0.0.2/32');
  await page.fill('#in-endpoint', 'vpn.example.com:51820');

  await expect(page.locator('#tool-output')).toContainText('PrivateKey', { timeout: 15000 });
  const out = await outputText(page);
  expectWireGuardKeys(out);
  expect(out).toContain('[Interface]');
  expect(out).toContain('Address = 10.0.0.2/32');
  expect(out).toContain('Endpoint = vpn.example.com:51820');
  expect(out).toContain('# PublicKey = ');
});

test('wireguard-keygen deep-link supports JSON, two pairs, IPv6, and no preshared key', async ({ page }) => {
  await page.goto(
    tool +
      '?pairs=2&preshared_key=false&format=json&address=' +
      encodeURIComponent('10.0.0.2/32, fd00::2/128') +
      '&endpoint=' +
      encodeURIComponent('[fd00::1]:51820'),
  );

  await expect(page.locator('#in-pairs')).toHaveValue('2', { timeout: 15000 });
  await expect(page.locator('#in-preshared_key')).not.toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('json');
  const parsed = JSON.parse(await outputText(page));
  expect(parsed.key_pairs).toHaveLength(2);
  for (const item of parsed.key_pairs) {
    expect(item.private_key).toMatch(new RegExp(`^${keyRe}$`));
    expect(item.public_key).toMatch(new RegExp(`^${keyRe}$`));
    expect(item.preshared_key).toBeNull();
    expect(item.config).toContain('Address = 10.0.0.2/32, fd00::2/128');
    expect(item.config).toContain('Endpoint = [fd00::1]:51820');
  }
});

test('wireguard-keygen conf output omits endpoint when blank', async ({ page }) => {
  await page.goto(tool);
  await page.selectOption('#in-format', 'conf');
  await page.fill('#in-address', '10.7.0.1/24');
  await page.fill('#in-endpoint', '');

  await expect(page.locator('#tool-output')).toContainText('[Interface]', { timeout: 15000 });
  const out = await outputText(page);
  expect(out.startsWith('[Interface]')).toBe(true);
  expect(out).toContain('Address = 10.7.0.1/24');
  expect(out).not.toContain('PrivateKey   =');
  expect(out).not.toContain('Endpoint =');
});

test('wireguard-keygen page accepts exact 25-pair cap', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-address', '10.0.0.2/32');
  await page.fill('#in-pairs', '25');
  await page.uncheck('#in-preshared_key');

  await expect(page.locator('#tool-output')).toContainText('# ---- key pair 25 of 25 ----', { timeout: 15000 });
  const out = await outputText(page);
  expectWireGuardKeys(out, { preshared: false, count: 25 });
});

test('wireguard-keygen page surfaces validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-address', '10.0.0.2');

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 15000 });
  await expect(out).toContainText('no prefix length');
});
