import { test, expect } from './fixtures';

async function runWasm(
  page,
  grades: string,
  scale = '4.0',
  customScale = '',
  gradeFormat = 'auto',
  defaultCredits = '1',
  honorsBonus = '0.5',
  apBonus = '1.0',
  priorGpa = '0',
  priorCredits = '0',
  skipNongraded = 'true',
  decimals = '2',
  output = 'report',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/grade-to-gpa-parser/gizza_ai_grade_to_gpa_parser_web.js');
    await mod.default('/tools/grade-to-gpa-parser/gizza_ai_grade_to_gpa_parser_web_bg.wasm');
    return mod.run(
      args.grades,
      args.scale,
      args.customScale,
      args.gradeFormat,
      args.defaultCredits,
      args.honorsBonus,
      args.apBonus,
      args.priorGpa,
      args.priorCredits,
      args.skipNongraded,
      args.decimals,
      args.output,
    );
  }, { grades, scale, customScale, gradeFormat, defaultCredits, honorsBonus, apBonus, priorGpa, priorCredits, skipNongraded, decimals, output });
}

test('grade-to-gpa-parser wasm computes a credit-weighted GPA exactly', async ({ page }) => {
  await page.goto('/tools/grade-to-gpa-parser/');
  await page.waitForSelector('#in-grades');

  const report = await runWasm(page, 'Biology: A- 4\nMath: B 3\nArt: C 1');
  expect(report).toContain('GPA: 3.23');
  expect(report).toContain('Grade points: 25.80');
  expect(report).toContain('Credits counted: 8.00');
  expect(report).toContain('Biology — A- → 3.70 × 4.00 credits = 14.80');
});

test('grade-to-gpa-parser wasm covers scales, weighted tags, non-default checkbox, and json', async ({ page }) => {
  await page.goto('/tools/grade-to-gpa-parser/');
  await page.waitForSelector('#in-grades');

  await expect(runWasm(page, 'A+', '4.3')).resolves.toContain('GPA: 4.30');
  await expect(runWasm(page, 'AP History: A 3\nHonors Chem: B 3\nAP Art: F 3')).resolves.toContain('GPA: 2.83');
  await expect(runWasm(page, 'Biology: A 4\nYoga: W 2', '4.0', '', 'auto', '1', '0.5', '1.0', '0', '0', 'false'))
    .rejects.toThrow(/non-graded mark W/);

  const json = JSON.parse(await runWasm(page, 'Biology: A- 4', '4.0', '', 'auto', '1', '0.5', '1.0', '0', '0', 'true', '2', 'json'));
  expect(json).toMatchObject({ gpa: 3.7, credits: 4, courses_counted: 1 });
  expect(json.courses[0]).toMatchObject({ course: 'Biology', grade: 'A-' });
});

test('grade-to-gpa-parser page renders output from controls', async ({ page }) => {
  await page.goto('/tools/grade-to-gpa-parser/');
  await page.fill('#in-grades', 'A, B+, C-');
  await page.selectOption('#in-scale', '4.0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('GPA: 3.00', { timeout: 15_000 });
  await expect(out).toContainText('Courses counted: 3');
});

test('grade-to-gpa-parser deep-link prefills cumulative GPA and renders', async ({ page }) => {
  const params = new URLSearchParams({
    grades: 'A 10',
    scale: '4.0',
    custom_scale: '',
    grade_format: 'auto',
    default_credits: '1',
    honors_bonus: '0.5',
    ap_bonus: '1.0',
    prior_gpa: '3.0',
    prior_credits: '30',
    skip_nongraded: 'true',
    decimals: '2',
    output: 'report',
  });

  await page.goto(`/tools/grade-to-gpa-parser/?${params.toString()}`);
  await expect(page.locator('#in-grades')).toHaveValue('A 10', { timeout: 15_000 });
  await expect(page.locator('#in-prior_gpa')).toHaveValue('3.0');
  await expect(page.locator('#in-prior_credits')).toHaveValue('30');
  await expect(page.locator('#tool-output')).toContainText('Cumulative GPA: 3.25 over 40.00 credits', { timeout: 15_000 });
});
