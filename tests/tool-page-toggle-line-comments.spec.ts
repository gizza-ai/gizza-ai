import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  code: string,
  language = 'auto',
  mode = 'toggle',
  marker = '',
  spaceAfterMarker = 'true',
  align = 'indent',
  commentBlankLines = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/toggle-line-comments/gizza_ai_toggle_line_comments_web.js');
    await mod.default('/tools/toggle-line-comments/gizza_ai_toggle_line_comments_web_bg.wasm');
    return mod.run(
      args.code,
      args.language,
      args.mode,
      args.marker,
      args.spaceAfterMarker,
      args.align,
      args.commentBlankLines,
    );
  }, {
    code,
    language,
    mode,
    marker,
    spaceAfterMarker,
    align,
    commentBlankLines,
  });
}

test('toggle-line-comments page comments JavaScript exactly', async ({ page }) => {
  await page.goto('/tools/toggle-line-comments/');
  await setTextarea(page, '#in-code', 'const a = 1;\nconst b = 2;');
  await page.selectOption('#in-language', 'javascript');
  await page.selectOption('#in-mode', 'comment');

  await expect(page.locator('#tool-output')).toHaveText('// const a = 1;\n// const b = 2;', { timeout: 15_000 });
});

test('toggle-line-comments deep-link prefills controls and uncomments Python', async ({ page }) => {
  const params = new URLSearchParams({
    code: '# x = 1\n# y = 2',
    language: 'python',
    mode: 'toggle',
    marker: '',
    space_after_marker: 'true',
    align: 'indent',
    comment_blank_lines: 'false',
  });

  await page.goto(`/tools/toggle-line-comments/?${params.toString()}`);
  await expect(page.locator('#in-code')).toHaveValue('# x = 1\n# y = 2', { timeout: 15_000 });
  await expect(page.locator('#in-language')).toHaveValue('python');
  await expect(page.locator('#in-mode')).toHaveValue('toggle');
  await expect(page.locator('#in-space_after_marker')).toBeChecked();
  await expect(page.locator('#in-comment_blank_lines')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('x = 1\ny = 2', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool toggle-line-comments');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('toggle-line-comments wasm covers language enum values', async ({ page }) => {
  await page.goto('/tools/toggle-line-comments/');
  await page.waitForSelector('#in-code');

  const cases: Array<[string, string, string]> = [
    ['javascript', 'x();', '// x();'],
    ['typescript', 'let x: number = 1;', '// let x: number = 1;'],
    ['java', 'System.out.println(x);', '// System.out.println(x);'],
    ['csharp', 'Console.WriteLine(x);', '// Console.WriteLine(x);'],
    ['c', 'printf("x");', '// printf("x");'],
    ['cpp', 'std::cout << x;', '// std::cout << x;'],
    ['go', 'fmt.Println(x)', '// fmt.Println(x)'],
    ['rust', 'let x = 1;', '// let x = 1;'],
    ['swift', 'print(x)', '// print(x)'],
    ['kotlin', 'println(x)', '// println(x)'],
    ['scala', 'println(x)', '// println(x)'],
    ['php', 'echo $x;', '// echo $x;'],
    ['python', 'x = 1', '# x = 1'],
    ['ruby', 'puts x', '# puts x'],
    ['perl', 'print $x;', '# print $x;'],
    ['shell', 'echo x', '# echo x'],
    ['powershell', 'Write-Output x', '# Write-Output x'],
    ['yaml', 'x: 1', '# x: 1'],
    ['toml', 'x = 1', '# x = 1'],
    ['r', 'print(x)', '# print(x)'],
    ['dockerfile', 'FROM alpine', '# FROM alpine'],
    ['makefile', 'all:', '# all:'],
    ['sql', 'SELECT 1;', '-- SELECT 1;'],
    ['lua', 'print(x)', '-- print(x)'],
    ['haskell', 'main = print x', '-- main = print x'],
    ['ini', 'x=1', '; x=1'],
    ['clojure', '(println x)', ';; (println x)'],
    ['latex', '\\alpha', '% \\alpha'],
    ['vb', 'Dim x', `' Dim x`],
    ['batch', 'echo x', 'REM echo x'],
    ['css', 'body { color: red; }', '/* body { color: red; } */'],
    ['html', '<p>x</p>', '<!-- <p>x</p> -->'],
    ['xml', '<x>1</x>', '<!-- <x>1</x> -->'],
  ];

  for (const [language, input, expected] of cases) {
    await expect(runWasm(page, input, language, 'comment')).resolves.toBe(expected);
  }
});

test('toggle-line-comments covers booleans, custom marker, alignment and cap boundary', async ({ page }) => {
  await page.goto('/tools/toggle-line-comments/');
  await page.waitForSelector('#in-code');

  await expect(runWasm(page, '  x\n    y', 'python', 'comment', '', 'false', 'column0'))
    .resolves.toBe('#  x\n#    y');
  await expect(runWasm(page, 'a\n\nb', 'javascript', 'comment', '', 'true', 'indent', 'true'))
    .resolves.toBe('// a\n//\n// b');
  await expect(runWasm(page, 'todo', 'auto', 'comment', '@'))
    .resolves.toBe('@ todo');

  const atCap = 'x'.repeat(2_000_000);
  await expect(runWasm(page, atCap, 'python', 'comment', '', 'false')).resolves.toBe(`#${atCap}`);
  await expect(runWasm(page, `${atCap}x`, 'python')).rejects.toThrow(/too large/);
});
