import { test, expect } from './fixtures';

const LIB = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Application Version</key><string>12.12.4.1</string>
  <key>Music Folder</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/</string>
  <key>Tracks</key>
  <dict>
    <key>101</key>
    <dict>
      <key>Track ID</key><integer>101</integer>
      <key>Name</key><string>Let It Happen</string>
      <key>Artist</key><string>Tame Impala</string>
      <key>Album</key><string>Currents</string>
      <key>Genre</key><string>Electronic</string>
      <key>Year</key><integer>2015</integer>
      <key>Total Time</key><integer>467000</integer>
      <key>Size</key><integer>11200000</integer>
      <key>Play Count</key><integer>12</integer>
      <key>Rating</key><integer>100</integer>
      <key>Date Added</key><date>2016-02-03T10:00:00Z</date>
      <key>Location</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/Music/Tame%20Impala/Currents/01%20Let%20It%20Happen.m4a</string>
    </dict>
    <key>102</key>
    <dict>
      <key>Track ID</key><integer>102</integer>
      <key>Name</key><string>All I Need</string>
      <key>Artist</key><string>Air</string>
      <key>Album</key><string>Moon Safari</string>
      <key>Genre</key><string>Electronic</string>
      <key>Year</key><integer>1998</integer>
      <key>Total Time</key><integer>268000</integer>
      <key>Size</key><integer>6400000</integer>
      <key>Play Count</key><integer>30</integer>
      <key>Date Added</key><date>2014-07-01T09:30:00Z</date>
      <key>Location</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/Music/Air/Moon%20Safari/03%20All%20I%20Need.m4a</string>
    </dict>
  </dict>
  <key>Playlists</key>
  <array>
    <dict>
      <key>Name</key><string>Library</string>
      <key>Master</key><true/>
      <key>Playlist Persistent ID</key><string>AAAA0001</string>
      <key>Playlist Items</key>
      <array><dict><key>Track ID</key><integer>101</integer></dict></array>
    </dict>
    <dict>
      <key>Name</key><string>Late Night</string>
      <key>Playlist Persistent ID</key><string>AAAA0002</string>
      <key>Playlist Items</key>
      <array>
        <dict><key>Track ID</key><integer>102</integer></dict>
        <dict><key>Track ID</key><integer>101</integer></dict>
      </array>
    </dict>
  </array>
</dict>
</plist>`;

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function fillBase(page) {
  await page.locator('#in-library').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, LIB);
  await page.selectOption('#in-output', 'csv');
  await page.fill('#in-playlist', '');
  await page.fill('#in-fields', 'name,artist,album,genre,year,duration,location');
  await page.fill('#in-path_prefix', '');
  await page.selectOption('#in-path_style', 'original');
  await page.uncheck('#in-include_builtin');
  await page.selectOption('#in-sort_by', 'original');
  await page.fill('#in-limit', '0');
}

test('itunes-library-parser renders default CSV from pasted Library.xml', async ({ page }) => {
  await page.goto('/tools/itunes-library-parser/');
  await fillBase(page);

  await expect(page.locator('#tool-output')).toContainText('name,artist,album,genre,year,duration,location', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('Let It Happen,Tame Impala,Currents,Electronic,2015,7:47,/Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a');
  expect(text).toContain('All I Need,Air,Moon Safari,Electronic,1998,4:28,/Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a');
});

test('itunes-library-parser covers output enum choices with real output', async ({ page }) => {
  await page.goto('/tools/itunes-library-parser/');
  await fillBase(page);

  for (const [output, expected] of [
    ['csv', 'Let It Happen,Tame Impala'],
    ['tsv', 'Let It Happen\tTame Impala'],
    ['json', '"name": "Let It Happen"'],
    ['m3u', '/Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a'],
    ['m3u8', '#EXTINF:467,Tame Impala - Let It Happen'],
    ['playlists', 'Late Night,playlist,2,,AAAA0002'],
    ['summary', 'Tracks: 2'],
  ]) {
    await page.selectOption('#in-output', output);
    await expect(page.locator('#tool-output')).toContainText(expected, { timeout: 15000 });
  }
});

test('itunes-library-parser path styles, sorting, non-default checkbox and limit are applied', async ({ page }) => {
  await page.goto('/tools/itunes-library-parser/');
  await fillBase(page);

  await page.selectOption('#in-output', 'playlists');
  await page.check('#in-include_builtin');
  await expect(page.locator('#tool-output')).toContainText('Library,built-in,1,,AAAA0001', { timeout: 15000 });

  await page.selectOption('#in-output', 'csv');
  await page.fill('#in-fields', 'name,location');
  await page.selectOption('#in-sort_by', 'artist');
  await page.fill('#in-limit', '1');
  await expect(page.locator('#tool-output')).toContainText('All I Need,/Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a', { timeout: 15000 });
  expect(await outputText(page)).not.toContain('Let It Happen');

  await page.fill('#in-limit', '0');
  await page.fill('#in-path_prefix', 'D:\\Music');
  await page.selectOption('#in-path_style', 'windows');
  await expect(page.locator('#tool-output')).toContainText('D:\\Music\\Music\\Air\\Moon Safari\\03 All I Need.m4a', { timeout: 15000 });

  await page.selectOption('#in-path_style', 'filename');
  await expect(page.locator('#tool-output')).toContainText('All I Need,03 All I Need.m4a', { timeout: 15000 });
});

test('itunes-library-parser deep-link exports one playlist as extended M3U', async ({ page }) => {
  const params = new URLSearchParams({
    library: LIB,
    output: 'm3u8',
    playlist: 'Late Night',
    fields: 'name,artist,album,genre,year,duration,location',
    path_prefix: '',
    path_style: 'unix',
    include_builtin: 'false',
    sort_by: 'original',
    limit: '0',
  });

  await page.goto(`/tools/itunes-library-parser/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('m3u8');
  await expect(page.locator('#in-playlist')).toHaveValue('Late Night');
  await expect(page.locator('#tool-output')).toContainText('#EXTM3U', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('#EXTINF:268,Air - All I Need');
  expect(text.indexOf('All I Need')).toBeLessThan(text.indexOf('Let It Happen'));
});

test('itunes-library-parser exact limit cap boundary reports error above 100000', async ({ page }) => {
  await page.goto('/tools/itunes-library-parser/');
  await fillBase(page);
  await page.fill('#in-limit', '100001');

  await expect(page.locator('#tool-output')).toContainText('limit must be between 0 (no limit) and 100000', { timeout: 15000 });
});
