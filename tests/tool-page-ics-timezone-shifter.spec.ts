import { test, expect } from './fixtures';

const utcEvent = 'BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:demo@example\nDTSTART:20240310T140000Z\nDTEND:20240310T150000Z\nSUMMARY:Standup\nEND:VEVENT\nEND:VCALENDAR';

test('ics-timezone-shifter converts UTC event to Berlin TZID output', async ({ page }) => {
  await page.goto('/tools/ics-timezone-shifter/');
  await page.fill('#in-input', utcEvent);
  await page.fill('#in-to', 'Europe/Berlin');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('BEGIN:VTIMEZONE', { timeout: 15000 });
  await expect(out).toContainText('TZID:Europe/Berlin');
  await expect(out).toContainText('DTSTART;TZID=Europe/Berlin:20240310T150000');
  await expect(out).toContainText('DTEND;TZID=Europe/Berlin:20240310T160000');
});

test('ics-timezone-shifter supports deep-linked UTC output', async ({ page }) => {
  const input = encodeURIComponent(utcEvent);
  await page.goto(`/tools/ics-timezone-shifter/?input=${input}&from=UTC&to=Asia%2FTokyo&mode=convert&write_as=utc&include_vtimezone=false`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('DTSTART:20240310T140000Z', { timeout: 15000 });
  await expect(out).toContainText('DTEND:20240310T150000Z');
  await expect(out).not.toContainText('BEGIN:VTIMEZONE');
});

test('ics-timezone-shifter handles floating source times and relabel mode', async ({ page }) => {
  const floating = 'BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:ny@example\nDTSTART:20240710T090000\nDTEND:20240710T100000\nSUMMARY:Call\nEND:VEVENT\nEND:VCALENDAR';
  await page.goto('/tools/ics-timezone-shifter/');
  await page.fill('#in-input', floating);
  await page.fill('#in-from', 'America/New_York');
  await page.fill('#in-to', 'UTC');
  await page.selectOption('#in-write_as', 'utc');
  await page.uncheck('#in-include_vtimezone');
  let out = page.locator('#tool-output');
  await expect(out).toContainText('DTSTART:20240710T130000Z', { timeout: 15000 });

  await page.fill('#in-input', utcEvent);
  await page.fill('#in-to', 'America/New_York');
  await page.selectOption('#in-mode', 'relabel');
  await page.selectOption('#in-write_as', 'tzid');
  await page.check('#in-include_vtimezone');
  out = page.locator('#tool-output');
  await expect(out).toContainText('DTSTART;TZID=America/New_York:20240310T140000', { timeout: 15000 });
});
