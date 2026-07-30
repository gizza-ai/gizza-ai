import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test, expect } from './fixtures';

const CHAIN = readFileSync(
  path.resolve(__dirname, '../blocks/cert-chain-validate/core/tests/fixtures/chain-rsa.pem'),
  'utf8',
);
const ROOT = readFileSync(
  path.resolve(__dirname, '../blocks/cert-chain-validate/core/tests/fixtures/root-rsa.pem'),
  'utf8',
);
const INTERMEDIATE = readFileSync(
  path.resolve(__dirname, '../blocks/cert-chain-validate/core/tests/fixtures/int-rsa.pem'),
  'utf8',
);
const LEAF = readFileSync(
  path.resolve(__dirname, '../blocks/cert-chain-validate/core/tests/fixtures/leaf-rsa.pem'),
  'utf8',
);

test('cert-chain-validate page validates a PEM chain and reports certificate fields', async ({ page }) => {
  await page.goto('/tools/cert-chain-validate/');

  await page.fill('#in-chain_pem', CHAIN);

  await expect(page.locator('#tool-output')).toContainText('Certificate chain: VALID', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('Certificates checked: 3');
  await expect(page.locator('#tool-output')).toContainText('Ordering: leaf-to-root issuer/subject chain matches');
  await expect(page.locator('#tool-output')).toContainText('Subject: CN=cert-chain-validate.example');
});

test('cert-chain-validate supports deep-link params and shows order errors', async ({ page }) => {
  const qs = new URLSearchParams({ chain_pem: `${ROOT}\n${INTERMEDIATE}\n${LEAF}` });

  await page.goto('/tools/cert-chain-validate/?' + qs.toString());

  await expect(page.locator('#in-chain_pem')).toHaveValue(`${ROOT}\n${INTERMEDIATE}\n${LEAF}`, {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('issuer/subject mismatch', {
    timeout: 15000,
  });
});
