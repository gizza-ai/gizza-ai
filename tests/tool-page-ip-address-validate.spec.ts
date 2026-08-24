import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  addresses: string,
  family = 'any',
  output = 'report',
  ipv6Form = 'compressed',
  allowPrefix = 'true',
  allowPort = 'true',
  allowLeadingZeros = 'false',
  dedupe = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/ip-address-validate/gizza_ai_ip_address_validate_web.js');
    await mod.default('/tools/ip-address-validate/gizza_ai_ip_address_validate_web_bg.wasm');
    return mod.run(args.addresses, args.family, args.output, args.ipv6Form, args.allowPrefix, args.allowPort, args.allowLeadingZeros, args.dedupe);
  }, { addresses, family, output, ipv6Form, allowPrefix, allowPort, allowLeadingZeros, dedupe });
}

test('ip-address-validate page canonicalizes mixed addresses', async ({ page }) => {
  await page.goto('/tools/ip-address-validate/');
  await setTextarea(page, '#in-addresses', '192.168.1.1\n2001:0DB8::1\n192.168.1.256');
  await page.selectOption('#in-output', 'valid');

  await expect(page.locator('#tool-output')).toHaveText('192.168.1.1\n2001:db8::1', { timeout: 15_000 });
});

test('ip-address-validate deep-link prefills and expands IPv6', async ({ page }) => {
  const params = new URLSearchParams({
    addresses: '2001:db8::1\n::1',
    family: 'ipv6',
    output: 'valid',
    ipv6_form: 'expanded',
    allow_prefix: 'true',
    allow_port: 'true',
    allow_leading_zeros: 'false',
    dedupe: 'false',
  });
  await page.goto(`/tools/ip-address-validate/?${params.toString()}`);

  await expect(page.locator('#in-addresses')).toHaveValue('2001:db8::1\n::1', { timeout: 15_000 });
  await expect(page.locator('#in-family')).toHaveValue('ipv6');
  await expect(page.locator('#in-output')).toHaveValue('valid');
  await expect(page.locator('#in-ipv6_form')).toHaveValue('expanded');
  await expect(page.locator('#tool-output')).toContainText('2001:0db8:0000:0000:0000:0000:0000:0001', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool ip-address-validate');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('ip-address-validate wasm covers output enums and checkboxes', async ({ page }) => {
  await page.goto('/tools/ip-address-validate/');
  await page.waitForSelector('#in-addresses');

  await expect(runWasm(page, '192.168.001.010', 'any', 'valid', 'compressed', 'true', 'true', 'true'))
    .resolves.toBe('192.168.1.10');
  await expect(runWasm(page, '192.168.001.010', 'any', 'invalid'))
    .resolves.toContain('leading zeros');
  await expect(runWasm(page, '10.0.0.0/8', 'any', 'invalid', 'compressed', 'false'))
    .resolves.toContain('prefix suffix is not accepted');
  await expect(runWasm(page, '192.0.2.1:443', 'any', 'invalid', 'compressed', 'true', 'false'))
    .resolves.toContain('port suffix is not accepted');
  await expect(runWasm(page, '2001:DB8::1\n2001:0db8:0000:0000:0000:0000:0000:0001', 'ipv6', 'valid', 'compressed', 'true', 'true', 'false', 'true'))
    .resolves.toBe('2001:db8::1');

  await expect(runWasm(page, '192.168.1.1', 'any', 'table')).resolves.toContain('line,input,status,version');
  const jsonOut = await runWasm(page, '192.168.1.1', 'any', 'json');
  expect(JSON.parse(jsonOut)[0].canonical).toBe('192.168.1.1');
  await expect(runWasm(page, '192.168.1.256', 'any', 'invalid')).resolves.toContain('octet 4');
  await expect(runWasm(page, '192.168.1.1', 'any', 'summary')).resolves.toContain('type:private,1');
});

test('ip-address-validate covers family filters and exact cap', async ({ page }) => {
  await page.goto('/tools/ip-address-validate/');
  await page.waitForSelector('#in-addresses');

  await expect(runWasm(page, '8.8.8.8', 'ipv6', 'invalid')).resolves.toContain('family filter');
  const atCap = Array.from({ length: 5000 }, (_, i) => `192.0.2.${i % 250}`).join('\n');
  await expect(runWasm(page, atCap, 'ipv4', 'summary')).resolves.toContain('total,5000');
  await expect(runWasm(page, `${atCap}\n192.0.2.1`, 'ipv4')).rejects.toThrow(/too many addresses/);
});
