import { test, expect } from './fixtures';

const SAMPLE = `Call the dentist tomorrow at 3pm
Urgent: submit expense report Friday
Buy milk`;

const EXPECTED = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'PRODID:-//gizza-ai//text-to-reminders//EN',
  'CALSCALE:GREGORIAN',
  'BEGIN:VTODO',
  'UID:todo-1-20260302@text-to-reminders',
  'DTSTAMP:20260302T000000Z',
  'SUMMARY:Call the dentist',
  'DUE:20260303T150000',
  'BEGIN:VALARM',
  'ACTION:DISPLAY',
  'DESCRIPTION:Call the dentist',
  'TRIGGER;RELATED=END:-PT30M',
  'END:VALARM',
  'END:VTODO',
  'BEGIN:VTODO',
  'UID:todo-2-20260302@text-to-reminders',
  'DTSTAMP:20260302T000000Z',
  'SUMMARY:submit expense report',
  'DUE;VALUE=DATE:20260306',
  'PRIORITY:1',
  'BEGIN:VALARM',
  'ACTION:DISPLAY',
  'DESCRIPTION:submit expense report',
  'TRIGGER;RELATED=END:-PT30M',
  'END:VALARM',
  'END:VTODO',
  'BEGIN:VTODO',
  'UID:todo-3-20260302@text-to-reminders',
  'DTSTAMP:20260302T000000Z',
  'SUMMARY:Buy milk',
  'END:VTODO',
  'END:VCALENDAR',
].join('\r\n');

test('text-to-reminders page converts a dated brain-dump to an exact reminders file', async ({ page }) => {
  await page.goto('/tools/text-to-reminders/');
  await page.fill('#in-text', SAMPLE);
  await page.fill('#in-reference_date', '2026-03-02');
  await page.fill('#in-alarm_minutes', '30');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('SUMMARY:Call the dentist', { timeout: 15_000 });
  expect(await out.textContent()).toBe(EXPECTED);
});

test('text-to-reminders deep link can drop undated lines and disable priority detection', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'Buy milk\nUrgent renewal in 3 days',
    reference_date: '2026-03-02',
    detect_priority: 'false',
    include_undated: 'false',
    alarm_minutes: '0',
  });

  await page.goto(`/tools/text-to-reminders/?${params.toString()}`);
  await expect(page.locator('#in-detect_priority')).not.toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-include_undated')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('SUMMARY:Urgent renewal', { timeout: 15_000 });
  await expect(out).toContainText('DUE;VALUE=DATE:20260305');
  await expect(out).not.toContainText('SUMMARY:Buy milk');
  await expect(out).not.toContainText('PRIORITY:');
});
