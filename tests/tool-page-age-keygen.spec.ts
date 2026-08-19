import { test, expect } from './fixtures';

const IDENTITY = 'AGE-SECRET-KEY-1GQ9778VQXMMJVE8SK7J6VT8UJ4HDQAJUVSFCWCM02D8GEWQ72PVQ2Y5J33';
const RECIPIENT = 'age1t7rxyev2z3rw82stdlrrepyc39nvn86l5078zqkf5uasdy86jp6svpy7pa';

async function runWasm(
  page: any,
  format = 'recipient_only',
  comment = '',
  include_created = false,
  seed_or_identity = IDENTITY,
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/age-keygen/gizza_ai_age_keygen_web.js');
    await mod.default('/tools/age-keygen/gizza_ai_age_keygen_web_bg.wasm');
    return mod.run(
      args.format,
      args.comment,
      args.include_created,
      args.seed_or_identity,
    );
  }, { format, comment, include_created, seed_or_identity });
}

test('age-keygen wasm derives the known public recipient from an identity', async ({ page }) => {
  await page.goto('/tools/age-keygen/');
  await page.waitForSelector('#in-format');

  expect(await runWasm(page)).toBe(RECIPIENT);
  expect(await runWasm(page, 'identity_only')).toBe(IDENTITY);

  const json = JSON.parse(await runWasm(page, 'json', 'laptop backup key'));
  expect(json).toEqual({
    recipient: RECIPIENT,
    identity: IDENTITY,
    comment: 'laptop backup key',
  });
});

test('age-keygen page computes exact recipient output from the form', async ({ page }) => {
  await page.goto('/tools/age-keygen/');
  await page.selectOption('#in-format', 'recipient_only');
  await page.uncheck('#in-include_created');
  await page.fill('#in-seed_or_identity', IDENTITY);

  await expect(page.locator('#tool-output')).toHaveText(RECIPIENT, { timeout: 15_000 });
});

test('age-keygen deep link covers JSON output, comment, and checkbox off', async ({ page }) => {
  const params = new URLSearchParams({
    format: 'json',
    comment: 'ci deploy key',
    include_created: 'false',
    seed_or_identity: IDENTITY,
  });
  await page.goto(`/tools/age-keygen/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('json', { timeout: 15_000 });
  await expect(page.locator('#in-comment')).toHaveValue('ci deploy key');
  await expect(page.locator('#in-include_created')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(JSON.stringify({
    comment: 'ci deploy key',
    identity: IDENTITY,
    recipient: RECIPIENT,
  }, null, 2), { timeout: 15_000 });
});

test('age-keygen covers enum outputs, fresh random generation, comment cap, and CLI example', async ({ page }) => {
  await page.goto('/tools/age-keygen/');
  await page.waitForSelector('#in-format');

  const text = await runWasm(page, 'text', 'test key', false);
  expect(text).toBe(`# test key\n# public key: ${RECIPIENT}\n${IDENTITY}`);

  const fresh = JSON.parse(await runWasm(page, 'json', '', true, ''));
  expect(fresh.recipient).toMatch(/^age1[023456789acdefghjklmnpqrstuvwxyz]+$/);
  expect(fresh.identity).toMatch(/^AGE-SECRET-KEY-1[023456789ACDEFGHJKLMNPQRSTUVWXYZ]+$/);

  await expect(runWasm(page, 'text', 'x'.repeat(201), false, '')).rejects.toThrow(/comment is too long/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool age-keygen');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
