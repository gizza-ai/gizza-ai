import { test, expect } from './fixtures';

async function fillStreamEditor(page, opts: {
  text?: string;
  script?: string;
  quiet?: boolean;
  ignoreCase?: boolean;
  wholeBuffer?: boolean;
  regexFlavor?: string;
  lineEnding?: string;
  maxOutputLines?: string;
}) {
  if (opts.text !== undefined) await page.fill('#in-text', opts.text);
  if (opts.script !== undefined) await page.fill('#in-script', opts.script);
  if (opts.quiet !== undefined) opts.quiet ? await page.check('#in-quiet') : await page.uncheck('#in-quiet');
  if (opts.ignoreCase !== undefined) opts.ignoreCase ? await page.check('#in-ignore_case') : await page.uncheck('#in-ignore_case');
  if (opts.wholeBuffer !== undefined) opts.wholeBuffer ? await page.check('#in-whole_buffer') : await page.uncheck('#in-whole_buffer');
  if (opts.regexFlavor) await page.selectOption('#in-regex_flavor', opts.regexFlavor);
  if (opts.lineEnding) await page.selectOption('#in-line_ending', opts.lineEnding);
  if (opts.maxOutputLines !== undefined) await page.fill('#in-max_output_lines', opts.maxOutputLines);
}

test('stream-editor substitutes and deletes lines', async ({ page }) => {
  await page.goto('/tools/stream-editor/');
  await fillStreamEditor(page, {
    text: 'foo\n\nkeep foo\ndrop this',
    script: 's/foo/bar/g\n/drop/d\n/^[[:space:]]*$/d',
  });

  const output = page.locator('#tool-output');
  await expect(output).toHaveText('bar\nkeep bar', { timeout: 20000 });
});

test('stream-editor quiet mode prints matching lines only from a deep link', async ({ page }) => {
  const qs =
    '?text=' + encodeURIComponent('alpha\nbeta\ngamma') +
    '&script=' + encodeURIComponent('/a$/p') +
    '&quiet=true' +
    '&ignore_case=false' +
    '&whole_buffer=false' +
    '&regex_flavor=basic' +
    '&line_ending=lf' +
    '&max_output_lines=100000';

  await page.goto('/tools/stream-editor/' + qs);
  await expect(page.locator('#in-quiet')).toBeChecked({ timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('alpha\nbeta\ngamma', { timeout: 20000 });
});

test('stream-editor supports extended regex groups', async ({ page }) => {
  await page.goto('/tools/stream-editor/');
  await fillStreamEditor(page, {
    text: 'ID-123\nnope',
    script: 's/ID-([0-9]+)/item:\\1/',
    regexFlavor: 'extended',
  });

  await expect(page.locator('#tool-output')).toHaveText('item:123\nnope', { timeout: 20000 });
});

test('stream-editor reports sandboxed file commands as errors', async ({ page }) => {
  await page.goto('/tools/stream-editor/');
  await fillStreamEditor(page, {
    text: 'hello',
    script: '1r /etc/passwd',
  });

  const output = page.locator('#tool-output');
  await expect(output).toHaveClass(/error/, { timeout: 20000 });
  await expect(output).toContainText('not available');
});
