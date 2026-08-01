import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

// Base64 of a minimal QuickTime file (ftyp + moov/udta/©xyz) tagging San Francisco.
const SF_B64 =
  'AAAAFGZ0eXBxdCAgAAACAHF0ICAAAAAubW9vdgAAACZ1ZHRhAAAAHql4eXoAEhXHKzM3Ljc3NDktMTIyLjQxOTQv';
// Same file, as hex.
const SF_HEX =
  '00000014667479707174202000000200717420200000002e6d6f6f7600000026756474610000001ea978797a001215c72b33372e373734392d3132322e343139342f';

test('video-gps-metadata-extractor reads the ©xyz location from base64', async ({ page }) => {
  await page.goto('/tools/video-gps-metadata-extractor/');
  await page.fill('#in-input', SF_B64);
  await expect(page.locator('#tool-output')).toContainText('GPS location found: 1', {
    timeout: 15000,
  });
  const text = await output(page);
  expect(text).toContain('Latitude    37.7749');
  expect(text).toContain('Longitude   -122.4194');
  expect(text).toContain('openstreetmap.org');
});

test('video-gps-metadata-extractor deep-links hex input to JSON output', async ({ page }) => {
  await page.goto(
    `/tools/video-gps-metadata-extractor/?input=${SF_HEX}&input_format=hex&output=json`,
  );
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"count": 1', { timeout: 15000 });
  const text = await output(page);
  expect(text).toContain('"latitude": 37.7749');
  expect(text).toContain('"longitude": -122.4194');
});
