import { test, expect } from './fixtures';

const CSV = 'file,artist,album,track,title\ntrack01.mp3,Tame Impala,Currents,1,Let It Happen\ntrack02.mp3,Tame Impala,Currents,2,Nangs';
const TSV = 'file\tartist\talbum\ttrack\ttitle\na.flac\tSigur Rós\tÁgætis byrjun\t3\tSvefn-g-englar';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function fillDefaults(page) {
  await page.fill('#in-tracks', CSV);
  await page.selectOption('#in-input_format', 'csv');
  await page.fill('#in-pattern', '{artist}/{album}/{track} {title}');
  await page.fill('#in-track_padding', '2');
  await page.selectOption('#in-on_missing', 'unknown');
  await page.fill('#in-unknown_text', 'Unknown');
  await page.selectOption('#in-charset', 'windows');
  await page.fill('#in-replace_char', '_');
  await page.selectOption('#in-space_style', 'keep');
  await page.selectOption('#in-case_style', 'keep');
  await page.fill('#in-max_component', '100');
  await page.check('#in-keep_extension');
  await page.selectOption('#in-format', 'table');
}

test('music-file-renamer renders the default album folder plan', async ({ page }) => {
  await page.goto('/tools/music-file-renamer/');
  await fillDefaults(page);

  await expect(page.locator('#tool-output')).toContainText('2 tracks, 2 renames, 0 unchanged, 0 collisions, 0 skipped', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('track01.mp3  ->  Tame Impala/Currents/01 Let It Happen.mp3');
  expect(text).toContain('track02.mp3  ->  Tame Impala/Currents/02 Nangs.mp3');
});

test('music-file-renamer covers ascii, lowercase, hyphen spaces and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/music-file-renamer/');
  await page.fill('#in-tracks', TSV);
  await page.selectOption('#in-input_format', 'tsv');
  await page.fill('#in-pattern', '{artist}/{album}/{track}-{title}.{ext}');
  await page.fill('#in-base_dir', 'Library');
  await page.fill('#in-track_padding', '2');
  await page.selectOption('#in-charset', 'ascii');
  await page.fill('#in-replace_char', '-');
  await page.selectOption('#in-space_style', 'hyphen');
  await page.selectOption('#in-case_style', 'lower');
  await page.fill('#in-max_component', '80');
  await page.uncheck('#in-keep_extension');
  await page.selectOption('#in-format', 'list');

  await expect(page.locator('#tool-output')).toContainText('a.flac -> Library/sigur-ros/agaetis-byrjun/03-svefn-g-englar.flac', { timeout: 15000 });
});

test('music-file-renamer deep-link supports json input and shell output', async ({ page }) => {
  const params = new URLSearchParams({
    tracks: '{"tracks":[{"filename":"x.m4a","artist":"Air","album":"Moon Safari","track":"3","title":"All I Need"}]}',
    input_format: 'json',
    pattern: '{albumartist|artist}/{album}/{track} {title}',
    base_dir: '',
    track_padding: '2',
    on_missing: 'skip',
    unknown_text: 'Unknown',
    charset: 'windows',
    replace_char: '_',
    space_style: 'keep',
    case_style: 'keep',
    max_component: '100',
    keep_extension: 'true',
    format: 'sh',
  });

  await page.goto(`/tools/music-file-renamer/?${params.toString()}`);
  await expect(page.locator('#in-input_format')).toHaveValue('json');
  await expect(page.locator('#in-format')).toHaveValue('sh');
  await expect(page.locator('#tool-output')).toContainText("mv -n 'x.m4a' 'Air/Moon Safari/03 All I Need.m4a'", { timeout: 15000 });
});

test('music-file-renamer enum formats produce real output', async ({ page }) => {
  await page.goto('/tools/music-file-renamer/');
  await fillDefaults(page);

  for (const [format, expected] of [
    ['table', 'track01.mp3  ->  Tame Impala/Currents/01 Let It Happen.mp3'],
    ['list', 'track01.mp3 -> Tame Impala/Currents/01 Let It Happen.mp3'],
    ['csv', 'current_path,new_path,status'],
    ['json', '"renames": 2'],
    ['sh', 'mv -n'],
  ]) {
    await page.selectOption('#in-format', format);
    await expect(page.locator('#tool-output')).toContainText(expected, { timeout: 15000 });
  }
});

test('music-file-renamer exact max tracks boundary errors above 5000', async ({ page }) => {
  const lines = ['file,artist,album,track,title'];
  for (let i = 1; i <= 5001; i++) {
    lines.push(`t${i}.mp3,A,B,${i},Song ${i}`);
  }
  await page.goto('/tools/music-file-renamer/');
  await page.fill('#in-tracks', lines.join('\n'));
  await page.selectOption('#in-input_format', 'csv');
  await page.fill('#in-pattern', '{artist}/{album}/{track} {title}');

  await expect(page.locator('#tool-output')).toContainText('too many tracks: 5001 (max 5000 per run)', { timeout: 15000 });
});
