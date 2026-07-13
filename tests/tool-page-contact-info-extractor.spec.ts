import { test, expect } from './fixtures';

async function setMaybeSelect(page, selector: string, value: string) {
  const el = page.locator(selector);
  const tag = await el.evaluate((node) => node.tagName.toLowerCase());
  if (tag === 'select') {
    await el.selectOption(value);
  } else if (tag === 'input') {
    const type = await el.getAttribute('type');
    if (type === 'checkbox') {
      const checked = value === 'true';
      if ((await el.isChecked()) !== checked) await el.setChecked(checked);
    } else {
      await el.fill(value);
    }
  } else {
    await el.fill(value);
  }
}

test('contact-info-extractor page extracts emails and phone numbers', async ({ page }) => {
  await page.goto('/tools/contact-info-extractor/');
  await page.waitForSelector('#in-input');
  await page.fill('#in-input', 'Reach Alice at alice@corp.com or call +1 415 555 2671. Bob: bob@corp.com, (212) 555-0199.');
  await setMaybeSelect(page, '#in-types', 'both');
  await setMaybeSelect(page, '#in-dedupe', 'true');
  await setMaybeSelect(page, '#in-sort', 'first-seen');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('4 item(s): 2 email(s), 2 phone(s)');
  await expect(output).toContainText('alice@corp.com');
  await expect(output).toContainText('bob@corp.com');
  await expect(output).toContainText('+1 415 555 2671');
  await expect(output).toContainText('(212) 555-0199');
});

test('contact-info-extractor honors query params for emails-only sorted output', async ({ page }) => {
  await page.goto('/tools/contact-info-extractor/?input=zed%40x.com%20amy%40x.com%20bob%40x.com%20AMY%40x.com&types=emails&dedupe=true&sort=alphabetical');
  await page.waitForSelector('#in-input');
  await expect(page.locator('#in-input')).toHaveValue('zed@x.com amy@x.com bob@x.com AMY@x.com');
  await expect(page.locator('#in-types')).toHaveValue('emails');
  await expect(page.locator('#in-sort')).toHaveValue('alphabetical');
  const text = await page.locator('#tool-output').innerText();
  expect(text).toContain('3 item(s): 3 email(s), 0 phone(s)');
  expect(text.indexOf('amy@x.com')).toBeLessThan(text.indexOf('bob@x.com'));
  expect(text.indexOf('bob@x.com')).toBeLessThan(text.indexOf('zed@x.com'));
});

test('contact-info-extractor wasm export rejects bad options', async ({ page }) => {
  await page.goto('/tools/contact-info-extractor/');
  await page.waitForSelector('#in-input');
  const result = await page.evaluate(async () => {
    const mod = await import('/tools/contact-info-extractor/gizza_ai_contact_info_extractor_web.js');
    await mod.default('/tools/contact-info-extractor/gizza_ai_contact_info_extractor_web_bg.wasm');
    return mod.run('a@x.com 555-123-4567', 'phones', 'true', 'first-seen');
  });
  expect(result).toContain('1 item(s): 0 email(s), 1 phone(s)');
  expect(result).toContain('555-123-4567');

  await expect(page.evaluate(async () => {
    const mod = await import('/tools/contact-info-extractor/gizza_ai_contact_info_extractor_web.js');
    await mod.default('/tools/contact-info-extractor/gizza_ai_contact_info_extractor_web_bg.wasm');
    return mod.run('a@x.com', 'bad', 'true', 'first-seen');
  })).rejects.toThrow(/types/);
});
