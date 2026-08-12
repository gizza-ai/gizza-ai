import { test, expect } from './fixtures';

const TWO = `--- mft ---
Path,Created,LastModified
\\Users\\a\\evil.exe,2024-06-01 10:00:05,2024-06-01 10:00:09
=== evtx ===
TimeCreated,EventID,Computer
2024-06-01 10:00:01,4624,DC01`;

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('super-timeline-builder merges sections into exact sorted CSV', async ({ page }) => {
  await page.goto('/tools/super-timeline-builder/');
  await setTextarea(page, '#in-artifacts', TWO);

  await expect(page.locator('#tool-output')).toHaveText(`datetime,timestamp_desc,source,message
2024-06-01T10:00:01Z,TimeCreated,evtx,EventID=4624; Computer=DC01
2024-06-01T10:00:05Z,Created,mft,Path=\\Users\\a\\evil.exe
2024-06-01T10:00:09Z,LastModified,mft,Path=\\Users\\a\\evil.exe`, { timeout: 15_000 });
});

test('super-timeline-builder deep link covers l2tcsv, desc order, filters, and checkbox off paths', async ({ page }) => {
  const params = new URLSearchParams({
    artifacts: TWO,
    format: 'l2tcsv',
    order: 'desc',
    expand: 'false',
    dedupe: 'false',
    from: '2024-06-01T10:00:00Z',
    to: '2024-06-01T10:00:05Z',
    tz_offset: '0',
    drop_epoch_zero: 'true',
    delimiter: 'auto',
    limit: '100000',
  });
  await page.goto(`/tools/super-timeline-builder/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('l2tcsv', { timeout: 15_000 });
  await expect(page.locator('#in-order')).toHaveValue('desc');
  await expect(page.locator('#in-expand')).not.toBeChecked();
  await expect(page.locator('#in-dedupe')).not.toBeChecked();
  await expect(page.locator('#in-drop_epoch_zero')).toBeChecked();
  await expect(page.locator('#in-delimiter')).toHaveValue('auto');
  await expect(page.locator('#in-limit')).toHaveValue('100000');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('date,time,timezone,MACB,source,sourcetype,type,user,host,short,desc,version,filename,inode,notes,format,extra', { timeout: 15_000 });
  await expect(out).toContainText('06/01/2024,10:00:05,UTC,,MFT,mft,Created');
  await expect(out).toContainText('06/01/2024,10:00:01,UTC,,EVTX,evtx,TimeCreated');
  await expect(out).not.toContainText('LastModified');
});

test('super-timeline-builder covers TLN, explicit tab delimiter, and timezone offset', async ({ page }) => {
  await page.goto('/tools/super-timeline-builder/');
  await setTextarea(page, '#in-artifacts', '# evtx\nTimeCreated\tEventID\tComputer\n2024-06-01 12:00:00\t4624\tDC01');
  await page.selectOption('#in-format', 'tln');
  await page.selectOption('#in-delimiter', 'tab');
  await page.fill('#in-tz_offset', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Time|Source|Host|User|Description', { timeout: 15_000 });
  await expect(out).toContainText('1717236000|evtx|DC01||TimeCreated - EventID=4624; Computer=DC01');
});

test('super-timeline-builder generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/super-timeline-builder/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool super-timeline-builder');
  expect(cli).toContain('mft');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
