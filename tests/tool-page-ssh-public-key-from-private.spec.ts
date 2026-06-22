import { test, expect } from './fixtures';

// /tools/ssh-public-key-from-private/ derives the OpenSSH public-key line
// (id_*.pub format) from a private key, entirely in-browser (pure wasm).
// The EC P-256 fixture was generated with openssl and the expected output
// cross-checked byte-for-byte against `ssh-keygen -y -f key`.

const EC_PRIV =
  '-----BEGIN EC PRIVATE KEY-----\n' +
  'MHcCAQEEIJBz+m9vKv0pHNxy9R3g0fMQH1i7zDa4BT7Y8hTdZjVUoAoGCCqGSM49\n' +
  'AwEHoUQDQgAEH8EKWD1yDHzbyUFsJAznFyX2E/hu0XDwgGp9NvbobnJb1dcyINGm\n' +
  'Q0UN0FAFI4z/Cwadx+W4yK2k34y4x67U3A==\n' +
  '-----END EC PRIVATE KEY-----';

// `ssh-keygen -y -f` output for the key above (the whole single line).
const EC_PUB_LINE =
  'ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBB/BClg9cgx828lBbCQM5xcl9hP4btFw8IBqfTb26G5yW9XXMiDRpkNFDdBQBSOM/wsGncfluMitpN+MuMeu1Nw=';

test('ssh-public-key-from-private page derives the OpenSSH key (auto)', async ({ page }) => {
  await page.goto('/tools/ssh-public-key-from-private/');
  await page.fill('#in-input', EC_PRIV);
  await page.selectOption('#in-key_type', 'auto');
  await expect(page.locator('#tool-output')).toContainText(EC_PUB_LINE, { timeout: 15000 });
});

test('ssh-public-key-from-private page appends a comment via deep-link', async ({ page }) => {
  const qs =
    '?input=' +
    encodeURIComponent(EC_PRIV) +
    '&key_type=ec&der_format=hex&comment=' +
    encodeURIComponent('me@host');
  await page.goto('/tools/ssh-public-key-from-private/' + qs);
  await expect(page.locator('#tool-output')).toContainText(EC_PUB_LINE + ' me@host', {
    timeout: 15000,
  });
});
