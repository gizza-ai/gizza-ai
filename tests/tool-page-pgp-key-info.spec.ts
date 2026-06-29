import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

const PUB = fs.readFileSync(path.join(__dirname, 'fixtures/pgp-verify-pub.asc'), 'utf-8');

test('pgp-key-info page inspects an armored public key', async ({ page }) => {
  await page.goto('/tools/pgp-key-info/');
  await page.fill('#in-key', PUB);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"key_type": "public"', { timeout: 15000 });
  await expect(out).toContainText('"user_ids"');
  await expect(out).toContainText('Test <t@example.com>');
  await expect(out).toContainText('"fingerprint"');
  await expect(out).toContainText('"subkeys"');
});

test('pgp-key-info query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/pgp-key-info/?key=' + encodeURIComponent(PUB));
  await expect(page.locator('#in-key')).toHaveValue(PUB);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"key_id"', { timeout: 15000 });
  await expect(out).toContainText('"algorithm"');
});
