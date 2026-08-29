import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setExistingKey(page: any, value: string) {
  await page.locator('#in-existing_key').evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const ED25519_TEST_SEED = '8XreH6+LuIrTCrt0Gj1p8/SfKgzVHbT6SfyESLYd6Es=';
const ED25519_P_TAG = 'mi4oZe5oURig5G66mm5QpOArHluNiVwjh2Q1i5QvUK8=';

test('dkim-generate page rebuilds a deterministic Ed25519 DNS value', async ({ page }) => {
  await page.goto('/tools/dkim-generate/');
  await page.fill('#in-domain', 'example.test');
  await page.fill('#in-selector', 'mail');
  await page.selectOption('#in-key_type', 'ed25519');
  await page.selectOption('#in-output', 'dns_value');
  await page.check('#in-include_hash');
  await page.selectOption('#in-flags', 'none');
  await setExistingKey(page, ED25519_TEST_SEED);

  await expect(page.locator('#tool-output')).toContainText(
    `v=DKIM1; h=sha256; k=ed25519; p=${ED25519_P_TAG}`,
    { timeout: 15000 },
  );
});

test('dkim-generate page deep-link preserves selector and test flag', async ({ page }) => {
  await page.goto(
    '/tools/dkim-generate/?domain=' +
      encodeURIComponent('example.test') +
      '&selector=s1&key_type=ed25519&output=zone_file&include_hash=false&flags=y&existing_key=' +
      encodeURIComponent(ED25519_TEST_SEED),
  );

  await expect(page.locator('#in-domain')).toHaveValue('example.test', { timeout: 15000 });
  await expect(page.locator('#in-selector')).toHaveValue('s1');
  await expect(page.locator('#in-include_hash')).not.toBeChecked();
  await expect(page.locator('#in-flags')).toHaveValue('y');
  await expect(page.locator('#tool-output')).toContainText('s1._domainkey.example.test.', {
    timeout: 15000,
  });
  const text = await outText(page);
  expect(text).toContain(`"v=DKIM1; k=ed25519; t=y; p=${ED25519_P_TAG}"`);
});

test('dkim-generate page json output exposes the DNS host and key type', async ({ page }) => {
  await page.goto('/tools/dkim-generate/');
  await page.fill('#in-domain', 'https://mail.example.test/path');
  await page.fill('#in-selector', 'ed1');
  await page.selectOption('#in-key_type', 'ed25519');
  await page.selectOption('#in-output', 'json');
  await setExistingKey(page, ED25519_TEST_SEED);

  await expect(page.locator('#tool-output')).toContainText(
    '"name": "ed1._domainkey.mail.example.test"',
    { timeout: 15000 },
  );
  const text = await outText(page);
  expect(text).toContain('"key_type": "ed25519"');
  expect(text).toContain(ED25519_P_TAG);
});

test('dkim-generate page reports invalid domains helpfully', async ({ page }) => {
  await page.goto('/tools/dkim-generate/');
  await page.fill('#in-domain', 'localhost');
  await page.fill('#in-selector', 'mail');
  await page.selectOption('#in-key_type', 'ed25519');
  await page.selectOption('#in-output', 'dns_value');
  await setExistingKey(page, ED25519_TEST_SEED);

  await expect(page.locator('#tool-output')).toContainText('full domain name', { timeout: 15000 });
});
