import { test, expect } from './fixtures';

const SAMPLE_TGZ = 'H4sIAAAAAAAAA+2VsQrCMBRFO/sVBfealybNLLSji39QmhaLqYG2op9vEhG1Q0Ux6ZCcJRAe5MJ994bXndxEdkEKRqk+gVH0ej6IgGRACAAm6h5ATUQxtazLcB7GsldSStFW9cycGmsaF4LcwrX/+2Kb74qk43be0AZnhMz4zyb+Y6zWJUZ25Lzjuf/rOFcbsFpaRmAhTP65rAaLn8AP/c/S0P9OePp/qIWQyXgd//7Gx/4HMvE/pQyF/neBcT2+yF7w8Al4iMm/aE9HK8m/Y/qfsS/6n2Kdf2y5lwye5z8QCPjLDVPttHEAEgAA';

async function runWasm(
  page,
  input = SAMPLE_TGZ,
  inputFormat = 'base64',
  output = 'table',
  sort = 'archive',
  filter = '',
  includeDirs = 'true',
  timeFormat = 'iso',
  limit = '500',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/tar-archive-lister/gizza_ai_tar_archive_lister_web.js');
    await mod.default('/tools/tar-archive-lister/gizza_ai_tar_archive_lister_web_bg.wasm');
    return mod.run(
      args.input,
      args.inputFormat,
      args.output,
      args.sort,
      args.filter,
      args.includeDirs,
      args.timeFormat,
      args.limit,
    );
  }, { input, inputFormat, output, sort, filter, includeDirs, timeFormat, limit });
}

test('tar-archive-lister wasm renders the sample long listing exactly', async ({ page }) => {
  await page.goto('/tools/tar-archive-lister/');
  const out = await runWasm(page);
  expect(out).toBe('drwxr-xr-x alice/staff  0 2024-05-01 10:00:00 demo/\n-rw-r--r-- alice/staff  7 2024-05-01 10:00:00 demo/README.md\ndrwxr-xr-x alice/staff  0 2024-05-01 10:00:00 demo/docs/\n-rw-r--r-- alice/staff 12 2024-05-01 10:00:00 demo/docs/hello.txt\nlrwxrwxrwx alice/staff  0 2024-05-01 10:00:00 demo/link.txt -> docs/hello.txt\n\n5 of 5 member(s) listed (2 file(s), 2 director(y/ies), 1 other) — 19 byte(s) of content in a 4608 tar.gz stream');
});

test('tar-archive-lister wasm covers advertised enum choices and filter forms', async ({ page }) => {
  await page.goto('/tools/tar-archive-lister/');

  await expect(runWasm(page, SAMPLE_TGZ, 'base64', 'paths'))
    .resolves.toBe('demo/\ndemo/README.md\ndemo/docs/\ndemo/docs/hello.txt\ndemo/link.txt');

  await expect(runWasm(page, SAMPLE_TGZ, 'base64', 'paths', 'path', '*.txt', 'false'))
    .resolves.toBe('demo/docs/hello.txt\ndemo/link.txt');

  const csv = await runWasm(page, SAMPLE_TGZ, 'base64', 'csv', 'size', '', 'false', 'epoch', '2');
  expect(csv).toContain('path,type,size,mode,uid,gid,uname,gname,mtime,link_target,offset');
  expect(csv).toContain('demo/docs/hello.txt,file,12,0644,1000,1000,alice,staff,1714557600,,2048');

  const json = await runWasm(page, SAMPLE_TGZ, 'base64', 'json', 'archive', 'README', 'true', 'iso', '500');
  expect(json).toContain('"matched_entries": 1');
  expect(json).toContain('"path": "demo/README.md"');

  await expect(runWasm(page, 'not an archive')).rejects.toThrow(/invalid base64|smallest valid|not a tar archive/);
});

test('tar-archive-lister page renders exact output and deep-link prefills params', async ({ page }) => {
  await page.goto('/tools/tar-archive-lister/');
  await page.fill('#in-input', SAMPLE_TGZ);
  await page.selectOption('#in-output', 'paths');
  await expect(page.locator('#tool-output')).toHaveText('demo/\ndemo/README.md\ndemo/docs/\ndemo/docs/hello.txt\ndemo/link.txt', { timeout: 15_000 });

  const qs =
    '?input=' + encodeURIComponent(SAMPLE_TGZ) +
    '&input_format=base64' +
    '&output=paths' +
    '&sort=archive' +
    '&filter=' +
    '&include_dirs=true' +
    '&time_format=iso' +
    '&limit=500';
  await page.goto('/tools/tar-archive-lister/' + qs);
  await expect(page.locator('#in-input')).toHaveValue(SAMPLE_TGZ, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('paths');
  await expect(page.locator('#tool-output')).toHaveText('demo/\ndemo/README.md\ndemo/docs/\ndemo/docs/hello.txt\ndemo/link.txt', { timeout: 15_000 });
});
