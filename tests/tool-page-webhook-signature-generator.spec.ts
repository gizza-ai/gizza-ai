import { test, expect } from './fixtures';

const payload = '{"id":"evt_test","type":"payment_intent.succeeded"}';

async function setField(page: any, selector: string, value: string) {
  const loc = page.locator(selector);
  const tag = await loc.evaluate((el: HTMLElement) => el.tagName.toLowerCase());
  if (tag === 'textarea') {
    await loc.evaluate((el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, value);
  } else {
    await loc.fill(value);
  }
}

async function runWasm(
  page: any,
  args: {
    payload: string;
    secret: string;
    provider?: string;
    timestamp?: string;
    message_id?: string;
    url?: string;
    algorithm?: string;
    encoding?: string;
    secret_encoding?: string;
    template?: string;
    header_name?: string;
    signature_prefix?: string;
    output?: string;
  },
) {
  return await page.evaluate(async (a) => {
    const mod = await import('/tools/webhook-signature-generator/gizza_ai_webhook_signature_generator_web.js');
    await mod.default('/tools/webhook-signature-generator/gizza_ai_webhook_signature_generator_web_bg.wasm');
    return mod.run(
      a.payload,
      a.secret,
      a.provider ?? 'stripe',
      a.timestamp ?? '',
      a.message_id ?? '',
      a.url ?? '',
      a.algorithm ?? 'sha256',
      a.encoding ?? 'hex',
      a.secret_encoding ?? 'auto',
      a.template ?? '',
      a.header_name ?? '',
      a.signature_prefix ?? '',
      a.output ?? 'all',
    );
  }, args);
}

test('webhook-signature-generator wasm matches published GitHub vector', async ({ page }) => {
  await page.goto('/tools/webhook-signature-generator/');
  await page.waitForSelector('#in-payload');

  const out = await runWasm(page, {
    payload: 'Hello, World!',
    secret: "It's a Secret to Everybody",
    provider: 'github',
    output: 'headers',
  });
  expect(out).toContain('X-Hub-Signature-256: sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17');
  expect(out).toContain('X-Hub-Signature: sha1=01dc10d0c83e72ed246219cdd91669667fe2ca59');
});

test('webhook-signature-generator wasm covers provider, output, and custom encoding enums', async ({ page }) => {
  await page.goto('/tools/webhook-signature-generator/');
  await page.waitForSelector('#in-payload');

  await expect(runWasm(page, {
    payload,
    secret: 'whsec_test_secret',
    provider: 'stripe',
    timestamp: '1700000000',
    output: 'header',
  })).resolves.toBe('t=1700000000,v1=1fa069bafb0fb0ec61ca8e80964ba68904130b9623f205d87d85ad75d5de7d02');

  await expect(runWasm(page, {
    payload: 'body',
    secret: 'k',
    provider: 'custom',
    timestamp: '1700000000',
    algorithm: 'sha512',
    encoding: 'base64url',
    secret_encoding: 'text',
    template: '{timestamp}|{payload}',
    header_name: 'X-My-Sig',
    signature_prefix: 'v1=',
    output: 'headers',
  })).resolves.toMatch(/^X-My-Sig: v1=/);

  await expect(runWasm(page, {
    payload: '{"test": 2432232314}',
    secret: 'whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw',
    provider: 'standard-webhooks',
    timestamp: '1614265330',
    message_id: 'msg_p5jXN8AQM9LWM0D4loKWxJek',
    output: 'headers',
  })).resolves.toContain('webhook-signature: v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=');
});

test('webhook-signature-generator page renders exact header output', async ({ page }) => {
  await page.goto('/tools/webhook-signature-generator/');
  await setField(page, '#in-payload', payload);
  await setField(page, '#in-secret', 'whsec_test_secret');
  await page.selectOption('#in-provider', 'stripe');
  await setField(page, '#in-timestamp', '1700000000');
  await page.selectOption('#in-output', 'header');

  await expect(page.locator('#tool-output')).toContainText('t=1700000000,v1=1fa069bafb0fb0ec61ca8e80964ba68904130b9623f205d87d85ad75d5de7d02', { timeout: 15_000 });
});

test('webhook-signature-generator deep-link prefills params and emits cURL without branding', async ({ page }) => {
  const params = new URLSearchParams({
    payload: 'Hello, World!',
    secret: "It's a Secret to Everybody",
    provider: 'github',
    timestamp: '',
    message_id: '',
    url: 'https://example.com/webhook',
    algorithm: 'sha256',
    encoding: 'hex',
    secret_encoding: 'auto',
    template: '',
    header_name: '',
    signature_prefix: '',
    output: 'curl',
  });

  await page.goto(`/tools/webhook-signature-generator/?${params.toString()}`);
  await expect(page.locator('#in-payload')).toHaveValue('Hello, World!', { timeout: 15_000 });
  await expect(page.locator('#in-provider')).toHaveValue('github');
  await expect(page.locator('#tool-output')).toContainText("curl -X POST 'https://example.com/webhook'", { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('X-Hub-Signature-256: sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool webhook-signature-generator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('webhook-signature-generator wasm returns actionable errors', async ({ page }) => {
  await page.goto('/tools/webhook-signature-generator/');
  await page.waitForSelector('#in-payload');

  await expect(runWasm(page, { payload: 'p', secret: 's', provider: 'square' })).rejects.toThrow(/url is required/);
  await expect(runWasm(page, { payload: 'p', secret: 's', provider: 'stripe', timestamp: '1700000000000' })).rejects.toThrow(/milliseconds/);
});
