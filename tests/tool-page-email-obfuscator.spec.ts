import { test, expect } from './fixtures';

// /tools/email-obfuscator/ encodes an email into scraper-resistant HTML
// (pure wasm). The output is a literal HTML string shown via textContent, so we
// assert on the source string, not rendered DOM.

test('email-obfuscator entity-encodes with a mailto link by default', async ({ page }) => {
  await page.goto('/tools/email-obfuscator/');
  await page.fill('#in-email', 'you@example.com');
  const out = page.locator('#tool-output');
  // Decimal entity for 'y' is &#121;, and a mailto: anchor is wrapped.
  await expect(out).toContainText('<a href=', { timeout: 15000 });
  await expect(out).toContainText('&#121;');
  // No literal address leaks into the source.
  await expect(out).not.toContainText('you@example.com');
});

test('email-obfuscator hex entities, no link', async ({ page }) => {
  await page.goto('/tools/email-obfuscator/');
  await page.fill('#in-email', 'a@b.io');
  await page.selectOption('#in-entity_style', 'hex');
  // link checkbox is checked by default — turn it off.
  await page.uncheck('#in-link');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('&#x61;', { timeout: 15000 });
  await expect(out).not.toContainText('<a');
});

test('email-obfuscator js mode builds from char codes', async ({ page }) => {
  await page.goto('/tools/email-obfuscator/');
  await page.fill('#in-email', 'joe@x.org');
  await page.selectOption('#in-mode', 'js');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('String.fromCharCode', { timeout: 15000 });
  await expect(out).toContainText('<noscript>');
  await expect(out).toContainText('106,111,101');
});

test('email-obfuscator query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/email-obfuscator/?email=' +
      encodeURIComponent('bob@one.com') +
      '&mode=rot13&link_text=' +
      encodeURIComponent('Email us'),
  );
  await expect(page.locator('#in-email')).toHaveValue('bob@one.com', {
    timeout: 15000,
  });
  await expect(page.locator('#in-mode')).toHaveValue('rot13');
  const out = page.locator('#tool-output');
  // rot13("mailto:bob@one.com") = "znvygb:obo@bar.pbz"; visible text "Email us".
  await expect(out).toContainText('znvygb:obo@bar.pbz', { timeout: 15000 });
  await expect(out).toContainText('Email us');
});
