import { test, expect } from './fixtures';

const SAMPLE = [
  '$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47',
  '$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A',
].join('\n');

test('nmea-to-csv page converts merged GGA/RMC cycle to real CSV output', async ({ page }) => {
  await page.goto('/tools/nmea-to-csv/');
  await page.fill('#in-nmea', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1994-03-23T12:35:19Z,12:35:19,48.1173,11.516667,545.4,gps,8,0.9,22.4,84.4', { timeout: 15000 });
});

test('nmea-to-csv deep-link applies delimiter, units, checkbox and coordinate params', async ({ page }) => {
  await page.goto('/tools/nmea-to-csv/?delimiter=semicolon&altitude_unit=ft&speed_unit=kmh&coordinates=decimal&validate_checksum=true&header=true');
  await page.fill('#in-nmea', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1994-03-23T12:35:19Z;12:35:19;48.1173;11.516667;1789.37;gps;8;0.9;41.48;84.4', { timeout: 15000 });
});

test('nmea-to-csv can omit the header row', async ({ page }) => {
  await page.goto('/tools/nmea-to-csv/?header=false');
  await page.fill('#in-nmea', '$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('12:35:19,48.1173,11.516667,545.4,gps,8,0.9,,', { timeout: 15000 });
  await expect(out).not.toContainText('time,latitude,longitude');
});
