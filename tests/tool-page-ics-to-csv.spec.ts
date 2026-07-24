import { test, expect } from './fixtures';

const sample =
  'BEGIN:VCALENDAR\n' +
  'BEGIN:VEVENT\n' +
  'UID:evt-1@example.com\n' +
  'SUMMARY:Team Standup\n' +
  'DTSTART:20240309T081530Z\n' +
  'DTEND:20240309T083000Z\n' +
  'LOCATION:Room 4\n' +
  'DESCRIPTION:Daily sync\n' +
  'END:VEVENT\n' +
  'END:VCALENDAR';

const expectedDefault =
  'summary,start,end,location,description,uid\n' +
  'Team Standup,2024-03-09T08:15:30Z,2024-03-09T08:30:00Z,Room 4,Daily sync,evt-1@example.com';

test('ics-to-csv page converts an event to exact CSV', async ({ page }) => {
  await page.goto('/tools/ics-to-csv/');
  await page.fill('#in-ics', sample);

  await expect(page.locator('#tool-output')).toHaveText(expectedDefault, {
    timeout: 15000,
  });
});

test('ics-to-csv page handles non-default delimiter/header/date/toggles', async ({ page }) => {
  await page.goto('/tools/ics-to-csv/');
  await page.fill('#in-ics', sample);
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.uncheck('#in-header');
  await page.selectOption('#in-date_format', 'raw');
  await page.uncheck('#in-include_location');
  await page.uncheck('#in-include_description');

  await expect(page.locator('#tool-output')).toHaveText(
    'Team Standup;20240309T081530Z;20240309T083000Z;evt-1@example.com',
    { timeout: 15000 },
  );
});

test('ics-to-csv query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/ics-to-csv/?ics=' +
      encodeURIComponent(sample) +
      '&delimiter=tab&date_format=raw&include_description=false',
  );

  await expect(page.locator('#in-ics')).toHaveValue(sample, { timeout: 15000 });
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-date_format')).toHaveValue('raw');
  await expect(page.locator('#in-include_description')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'summary\tstart\tend\tlocation\tuid\n' +
      'Team Standup\t20240309T081530Z\t20240309T083000Z\tRoom 4\tevt-1@example.com',
    { timeout: 15000 },
  );
});
