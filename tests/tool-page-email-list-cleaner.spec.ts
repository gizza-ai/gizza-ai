import { test, expect } from './fixtures';

// /tools/email-list-cleaner/ validates, normalizes, and dedupes pasted lists.
test('email-list-cleaner page reports valid, duplicate, and invalid rows', async ({ page }) => {
  await page.goto('/tools/email-list-cleaner/');
  await page.fill('#in-emails', 'Alice@example.com\nBob <bob@example.com>\nalice@example.com\nnot-an-email');
  await expect(page.locator('#tool-output')).toContainText('Entries processed: 4', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Valid unique: 2');
  await expect(page.locator('#tool-output')).toContainText('Duplicates removed: 1');
  await expect(page.locator('#tool-output')).toContainText('Invalid: 1');
  await expect(page.locator('#tool-output')).toContainText('alice@example.com');
  await expect(page.locator('#tool-output')).toContainText('bob@example.com');
  await expect(page.locator('#tool-output')).toContainText('not-an-email');
});

test('email-list-cleaner page honors deep-linked clean sorted output', async ({ page }) => {
  await page.goto('/tools/email-list-cleaner/?emails=zeta%40example.com%2C%20alpha%40example.com%3B%20zeta%40example.com&sort=alpha&format=clean');
  await expect(page.locator('#tool-output')).toHaveText('alpha@example.com\nzeta@example.com', { timeout: 15000 });
});

test('email-list-cleaner page folds Gmail aliases when requested', async ({ page }) => {
  await page.goto('/tools/email-list-cleaner/?emails=john.doe%2Bnews%40gmail.com%0Ajohndoe%40gmail.com%0AJohnDoe%40googlemail.com&canonicalize=true&format=report');
  await expect(page.locator('#tool-output')).toContainText('Entries processed: 3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Valid unique: 1');
  await expect(page.locator('#tool-output')).toContainText('Duplicates removed: 2');
  await expect(page.locator('#tool-output')).toContainText('johndoe@gmail.com');
});
