import { test, expect } from './fixtures';

function writeU16Both(img: Uint8Array, off: number, v: number) {
  img[off] = v & 0xff; img[off + 1] = (v >> 8) & 0xff;
  img[off + 2] = (v >> 8) & 0xff; img[off + 3] = v & 0xff;
}
function writeU32Both(img: Uint8Array, off: number, v: number) {
  img[off] = v & 0xff; img[off + 1] = (v >> 8) & 0xff; img[off + 2] = (v >> 16) & 0xff; img[off + 3] = (v >> 24) & 0xff;
  img[off + 4] = (v >> 24) & 0xff; img[off + 5] = (v >> 16) & 0xff; img[off + 6] = (v >> 8) & 0xff; img[off + 7] = v & 0xff;
}
function writeDirRecord(img: Uint8Array, off: number, lba: number, dataLen: number, isDir: boolean, name: number[] | string) {
  const bytes = typeof name === 'string' ? [...name].map((c) => c.charCodeAt(0)) : name;
  let recLen = 33 + bytes.length;
  if (recLen % 2 === 1) recLen += 1;
  img[off] = recLen;
  writeU32Both(img, off + 2, lba);
  writeU32Both(img, off + 10, dataLen);
  img[off + 25] = isDir ? 0x02 : 0;
  img[off + 32] = bytes.length;
  bytes.forEach((b, i) => { img[off + 33 + i] = b; });
  return recLen;
}
function tinyIsoBase64() {
  const bs = 2048;
  const img = new Uint8Array(22 * bs);
  const rootLba = 18;
  const docsLba = 19;
  const pvd = 16 * bs;
  img[pvd] = 1;
  img.set([67, 68, 48, 48, 49], pvd + 1); // CD001
  img[pvd + 6] = 1;
  [...'TESTDISC'].forEach((c, i) => { img[pvd + 40 + i] = c.charCodeAt(0); });
  for (let i = pvd + 48; i < pvd + 72; i++) img[i] = 32;
  writeU32Both(img, pvd + 80, 22);
  writeU16Both(img, pvd + 128, bs);
  writeDirRecord(img, pvd + 156, rootLba, bs, true, [0]);
  const term = 17 * bs;
  img[term] = 255;
  img.set([67, 68, 48, 48, 49], term + 1);
  img[term + 6] = 1;
  let p = rootLba * bs;
  p += writeDirRecord(img, p, rootLba, bs, true, [0]);
  p += writeDirRecord(img, p, rootLba, bs, true, [1]);
  p += writeDirRecord(img, p, docsLba, bs, true, 'DOCS');
  writeDirRecord(img, p, 20, 11, false, 'README.TXT;1');
  let q = docsLba * bs;
  q += writeDirRecord(img, q, docsLba, bs, true, [0]);
  q += writeDirRecord(img, q, rootLba, 2 * bs, true, [1]);
  writeDirRecord(img, q, 21, 4, false, 'A.TXT;1');
  let s = '';
  for (let i = 0; i < img.length; i += 0x8000) s += String.fromCharCode(...img.slice(i, i + 0x8000));
  return btoa(s);
}

test('iso9660-toc-list page renders a real ISO directory tree', async ({ page }) => {
  await page.goto('/tools/iso9660-toc-list/');
  await page.fill('#in-data', tinyIsoBase64());
  await page.selectOption('#in-format', 'tree');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('/  (volume: TESTDISC)', { timeout: 15000 });
  await expect(out).toContainText('DOCS/');
  await expect(out).toContainText('README.TXT  (11 B)');
  await expect(out).toContainText('A.TXT  (4 B)');
  await expect(out).not.toContainText(';1');
});

test('iso9660-toc-list deep-links summary format and reports counts', async ({ page }) => {
  await page.goto('/tools/iso9660-toc-list/?format=summary');
  await page.fill('#in-data', tinyIsoBase64());

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Volume label:    TESTDISC', { timeout: 15000 });
  await expect(out).toContainText('Directories:     1');
  await expect(out).toContainText('Files:           2');
});
