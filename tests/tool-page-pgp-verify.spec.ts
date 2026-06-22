import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

// /tools/pgp-verify/ verifies an OpenPGP signature in-browser (pure wasm, rPGP).
// signature + public_key + message are all multiline <textarea>. Output is a
// pretty-printed JSON report with a `valid` flag.
//
// Fixtures were produced once with the gizza CLI:
//   generate-pgp-key-pair (ed25519) -> public key
//   pgp-sign mode=detached  over "hello pgp world"
//   pgp-sign mode=clearsign over "clear text here"
const FIX = path.join(__dirname, 'fixtures');
const PUB = fs.readFileSync(path.join(FIX, 'pgp-verify-pub.asc'), 'utf-8');
const DETACHED = fs.readFileSync(path.join(FIX, 'pgp-verify-detached.sig'), 'utf-8');
const CLEARSIGN = fs.readFileSync(path.join(FIX, 'pgp-verify-clearsign.sig'), 'utf-8');

test('pgp-verify page validates a correct detached signature', async ({ page }) => {
  await page.goto('/tools/pgp-verify/');
  await page.fill('#in-signature', DETACHED);
  await page.fill('#in-public_key', PUB);
  await page.fill('#in-message', 'hello pgp world');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": true', { timeout: 15000 });
  await expect(out).toContainText('"mode": "detached"');
  await expect(out).toContainText('signer_fingerprint');
  // Enriched metadata extracted from the key + signature.
  await expect(out).toContainText('signer_user_id');
  await expect(out).toContainText('hash_algorithm');
});

test('pgp-verify page rejects a tampered detached message', async ({ page }) => {
  await page.goto('/tools/pgp-verify/');
  await page.fill('#in-signature', DETACHED);
  await page.fill('#in-public_key', PUB);
  await page.fill('#in-message', 'hello pgp WORLD');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15000 });
});

test('pgp-verify page validates a clearsigned message and shows its text', async ({ page }) => {
  await page.goto('/tools/pgp-verify/');
  await page.fill('#in-signature', CLEARSIGN);
  await page.fill('#in-public_key', PUB);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": true', { timeout: 15000 });
  await expect(out).toContainText('"mode": "clearsign"');
  await expect(out).toContainText('clear text here');
});
