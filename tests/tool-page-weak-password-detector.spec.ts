import { test, expect } from './fixtures';

async function setMaybeCheckbox(page, selector: string, checked: boolean) {
  const el = page.locator(selector);
  if ((await el.isChecked()) !== checked) await el.setChecked(checked);
}

test('weak-password-detector page flags exact common passwords', async ({ page }) => {
  await page.goto('/tools/weak-password-detector/');
  await page.waitForSelector('#in-input');
  await page.fill('#in-input', '123456');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Verdict : WEAK — CRITICAL');
  await expect(output).toContainText('exact');
  await expect(output).toContainText('rank #1');
});

test('weak-password-detector honors query params and leetspeak default', async ({ page }) => {
  await page.goto('/tools/weak-password-detector/?input=P%40ssw0rd&case_sensitive=false&normalize_leet=true');
  await page.waitForSelector('#in-input');
  await expect(page.locator('#in-input')).toHaveValue('P@ssw0rd');
  await expect(page.locator('#in-normalize_leet')).toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('WEAK');
  await expect(output).toContainText('leetspeak');
  await expect(output).toContainText('password');
});

test('weak-password-detector checkbox states change matching behavior', async ({ page }) => {
  await page.goto('/tools/weak-password-detector/');
  await page.waitForSelector('#in-input');

  await page.fill('#in-input', 'PASSWORD');
  await setMaybeCheckbox(page, '#in-case_sensitive', true);
  await setMaybeCheckbox(page, '#in-normalize_leet', false);
  await expect(page.locator('#tool-output')).toContainText('not on the common-password list');

  await setMaybeCheckbox(page, '#in-case_sensitive', false);
  await expect(page.locator('#tool-output')).toContainText('case-insensitive');

  await page.fill('#in-input', 'P@ssw0rd');
  await setMaybeCheckbox(page, '#in-normalize_leet', false);
  await expect(page.locator('#tool-output')).toContainText('not on the common-password list');

  await setMaybeCheckbox(page, '#in-normalize_leet', true);
  await expect(page.locator('#tool-output')).toContainText('leetspeak');
});

test('weak-password-detector wasm export rejects empty input', async ({ page }) => {
  await page.goto('/tools/weak-password-detector/');
  await page.waitForSelector('#in-input');
  const result = await page.evaluate(async () => {
    const mod = await import('/tools/weak-password-detector/gizza_ai_weak_password_detector_web.js');
    await mod.default('/tools/weak-password-detector/gizza_ai_weak_password_detector_web_bg.wasm');
    return mod.run('cor6rect$horse!Battery9Staple', 'false', 'true');
  });
  expect(result).toContain('not on the common-password list');

  await expect(page.evaluate(async () => {
    const mod = await import('/tools/weak-password-detector/gizza_ai_weak_password_detector_web.js');
    await mod.default('/tools/weak-password-detector/gizza_ai_weak_password_detector_web_bg.wasm');
    return mod.run('', 'false', 'true');
  })).rejects.toThrow(/password is empty/);
});
