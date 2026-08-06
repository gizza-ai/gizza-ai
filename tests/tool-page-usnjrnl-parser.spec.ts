import { test, expect } from './fixtures';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Four synthetic USN_RECORD_V2 records: create notes.txt, the two halves of a
// draft.txt -> final.txt rename, and a delete of secret.tmp.
const JOURNAL_HEX = readFileSync(resolve(__dirname, 'fixtures/usnjrnl-parser.hex'), 'utf8').trim();

const EXPECTED_LIST = [
  '$UsnJrnl:$J — 4 records parsed, 3 matched, 3 shown',
  '  note: 1 rename pair merged into a single Rename row (pair_renames=true).',
  '2024-05-01T12:00:00Z  usn=4096  File create  notes.txt  [FILE_CREATE]',
  '2024-05-01T12:00:06Z  usn=4272  Rename  draft.txt -> final.txt  [RENAME_OLD_NAME | RENAME_NEW_NAME | CLOSE]',
  '2024-05-01T12:00:12Z  usn=4360  File delete  secret.tmp  [FILE_DELETE | CLOSE]',
].join('\n');

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('usnjrnl-parser page renders the exact event timeline', async ({ page }) => {
  await page.goto('/tools/usnjrnl-parser/');
  await page.fill('#in-data', JOURNAL_HEX);
  await page.selectOption('#in-mode', 'list');
  await expect(page.locator('#tool-output')).toContainText('4 records parsed', { timeout: 15000 });
  expect(await output(page)).toBe(EXPECTED_LIST);
});

test('usnjrnl-parser deep link pre-fills and runs summary mode', async ({ page }) => {
  await page.goto(
    `/tools/usnjrnl-parser/?data=${encodeURIComponent(JOURNAL_HEX)}&mode=summary&sort=usn&max_entries=200`,
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('$UsnJrnl:$J summary', { timeout: 15000 });
  await expect(out).toContainText('Records parsed         4');
  await expect(out).toContainText('USN range              4096 (0x1000) .. 4360 (0x1108)');
  await expect(out).toContainText('Time range (UTC)       2024-05-01T12:00:00Z .. 2024-05-01T12:00:12Z');
  await expect(out).toContainText('Distinct MFT entries   3');
});

test('usnjrnl-parser un-checking rename merging shows both halves', async ({ page }) => {
  await page.goto('/tools/usnjrnl-parser/');
  await page.fill('#in-data', JOURNAL_HEX);
  await page.selectOption('#in-mode', 'list');
  await expect(page.locator('#tool-output')).toContainText('3 matched', { timeout: 15000 });
  await page.uncheck('#in-pair_renames');
  await expect(page.locator('#tool-output')).toContainText('4 matched', { timeout: 15000 });
  const raw = await output(page);
  expect(raw).toContain('Rename (old name)  draft.txt');
  expect(raw).toContain('Rename (new name)  final.txt');
  expect(raw).not.toContain('draft.txt -> final.txt');
});

test('usnjrnl-parser change-class filter and CSV export deep-link', async ({ page }) => {
  await page.goto(
    `/tools/usnjrnl-parser/?data=${encodeURIComponent(JOURNAL_HEX)}&event=delete&mode=csv&max_entries=200`,
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('timestamp,usn,file_name', { timeout: 15000 });
  expect(await output(page)).toBe(
    'timestamp,usn,file_name,renamed_to,change,reasons,file_attributes,is_directory,file_entry,' +
      'file_sequence,parent_entry,parent_sequence,source_info,security_id,version,offset\n' +
      '2024-05-01T12:00:12Z,4360,secret.tmp,,File delete,FILE_DELETE | CLOSE,ARCHIVE,false,99,3,5,1,,256,2.0,240',
  );
});

test('usnjrnl-parser rejects non-journal bytes with an actionable message', async ({ page }) => {
  await page.goto('/tools/usnjrnl-parser/?data=' + 'ff'.repeat(64) + '&mode=report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('no USN records found in 64 bytes', { timeout: 15000 });
  await expect(out).toContainText('the 4-byte record length reads 4294967295');
});
