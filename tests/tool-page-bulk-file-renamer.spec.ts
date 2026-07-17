import { test, expect } from './fixtures';

async function setMaybeSelect(page, selector: string, value: string) {
  const el = page.locator(selector);
  const tag = await el.evaluate((node) => node.tagName.toLowerCase());
  if (tag === 'select') {
    await el.selectOption(value);
  } else if (tag === 'input') {
    const type = await el.getAttribute('type');
    if (type === 'checkbox') {
      const checked = value === 'true';
      if ((await el.isChecked()) !== checked) await el.setChecked(checked);
    } else {
      await el.fill(value);
    }
  } else {
    await el.fill(value);
  }
}

test('bulk-file-renamer page previews find/replace mappings', async ({ page }) => {
  await page.goto('/tools/bulk-file-renamer/');
  await page.waitForSelector('#in-filenames');
  await page.fill('#in-filenames', 'IMG_001.JPG\nIMG_002.JPG');
  await setMaybeSelect(page, '#in-mode', 'find_replace');
  await page.fill('#in-find', 'IMG');
  await page.fill('#in-replace', 'photo');
  await setMaybeSelect(page, '#in-preserve_extension', 'true');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('IMG_001.JPG -> photo_001.JPG');
  await expect(output).toContainText('IMG_002.JPG -> photo_002.JPG');
});

test('bulk-file-renamer honors query params for sequential numbering and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    filenames: 'a.txt\nb.txt',
    mode: 'sequential',
    find: '',
    replace: '',
    case_type: 'lower',
    pattern: 'same',
    start: '7',
    padding: '2',
    prefix: '',
    suffix: '',
    preserve_extension: 'false',
  });
  await page.goto(`/tools/bulk-file-renamer/?${params.toString()}`);
  await page.waitForSelector('#in-filenames');

  await expect(page.locator('#in-filenames')).toHaveValue('a.txt\nb.txt');
  await expect(page.locator('#in-mode')).toHaveValue('sequential');
  await expect(page.locator('#in-pattern')).toHaveValue('same');
  await expect(page.locator('#in-start')).toHaveValue('7');
  await expect(page.locator('#in-padding')).toHaveValue('2');
  await expect(page.locator('#in-preserve_extension')).not.toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('a.txt -> same');
  await expect(output).toContainText('b.txt -> same');
  await expect(output).toContainText('collision');
});

test('bulk-file-renamer wasm export supports regex and rejects invalid regex', async ({ page }) => {
  await page.goto('/tools/bulk-file-renamer/');
  await page.waitForSelector('#in-filenames');
  const result = await page.evaluate(async () => {
    const mod = await import('/tools/bulk-file-renamer/gizza_ai_bulk_file_renamer_web.js');
    await mod.default('/tools/bulk-file-renamer/gizza_ai_bulk_file_renamer_web_bg.wasm');
    return mod.run('2026-07-17-report.pdf', 'regex', '(\\d{4})-(\\d{2})-(\\d{2})', '${3}_${2}_${1}', 'lower', 'file-{n}', '1', '1', '', '', 'true');
  });
  expect(result).toContain('2026-07-17-report.pdf -> 17_07_2026-report.pdf');

  await expect(page.evaluate(async () => {
    const mod = await import('/tools/bulk-file-renamer/gizza_ai_bulk_file_renamer_web.js');
    await mod.default('/tools/bulk-file-renamer/gizza_ai_bulk_file_renamer_web_bg.wasm');
    return mod.run('a.txt', 'regex', '(', '', 'lower', 'file-{n}', '1', '1', '', '', 'true');
  })).rejects.toThrow(/invalid regular expression/);
});
