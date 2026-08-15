import { test, expect } from './fixtures';

const ZPUB = 'zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs';
const UPUB = 'upub5EFU65HtV5TeiSHmZZm7FUffBGy8UKeqp7vw43jYbvZPpoVsgU93oac7Wk3u6moKegAEWtGNF8DehrnHtv21XXEMYRUocHqguyjknFHYfgY';

async function fillZpub(page: any) {
  await page.fill('#in-xpub', ZPUB);
}

test('xpub-derive page derives BIP84 receive and change addresses', async ({ page }) => {
  await page.goto('/tools/xpub-derive/');
  await fillZpub(page);
  await page.selectOption('#in-chain', 'both');
  await page.fill('#in-count', '2');
  await page.fill('#in-start', '0');
  await page.selectOption('#in-address_type', 'auto');
  await page.selectOption('#in-format', 'table');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('network: mainnet', { timeout: 15000 });
  await expect(out).toContainText('address_type: p2wpkh');
  await expect(out).toContainText('m/0/0  bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu');
  await expect(out).toContainText('m/0/1  bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g');
  await expect(out).toContainText('m/1/0  bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el');
});

test('xpub-derive supports csv output and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/xpub-derive/');
  await fillZpub(page);
  await page.selectOption('#in-chain', 'change');
  await page.fill('#in-count', '1');
  await page.selectOption('#in-format', 'csv');
  await page.check('#in-include_public_key');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('chain,index,path,address,public_key', { timeout: 15000 });
  await expect(out).toContainText('change,0,m/1/0,bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el,03025324888e429ab8e3dbaf1f7802648b9cd01e9b418485c5fa4c1b9b5700e1a6');
});

test('xpub-derive deep link prefills testnet upub and derives wrapped-segwit address', async ({ page }) => {
  const params = new URLSearchParams({
    xpub: UPUB,
    chain: 'receive',
    count: '1',
    start: '0',
    address_type: 'auto',
    format: 'table',
    include_public_key: 'false',
  });
  await page.goto(`/tools/xpub-derive/?${params.toString()}`);

  await expect(page.locator('#in-xpub')).toHaveValue(UPUB, { timeout: 15000 });
  await expect(page.locator('#in-chain')).toHaveValue('receive');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('network: testnet', { timeout: 15000 });
  await expect(out).toContainText('address_type: p2sh_p2wpkh');
  await expect(out).toContainText('2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2');
});
