import { test, expect } from './fixtures';

// /tools/resume-to-json/ extracts a pasted plain-text resume into the JSON
// Resume v1.0.0 schema, or validates an existing resume.json (pure wasm).
const SAMPLE =
  'Jane Doe\nSenior Software Engineer\nSan Francisco, CA | jane.doe@example.com | (555) 123-4567 | linkedin.com/in/janedoe\n\n' +
  'Summary\nEngineer with 8 years of experience building distributed systems.\n\n' +
  'Experience\n\nSenior Software Engineer — Acme Corp\nJan 2020 – Present | San Francisco, CA\n- Led migration to a service mesh across 40 services\n- Cut p99 latency by 45%\n\n' +
  'Software Engineer — Beta Labs\nJun 2016 – Dec 2019\n- Built the billing pipeline in Rust\n\n' +
  'Education\n\nB.S. in Computer Science — State University\n2012 – 2016\nGPA: 3.8/4.0\n\n' +
  'Skills\nLanguages: Rust, Python, TypeScript\n\nLanguages\nEnglish (Native), Spanish (Professional)';

const INVALID_DOC =
  '{ "basics": { "name": "Jane Doe", "email": "jane@example.com" }, "work": [ { "name": "Acme Corp", "position": "Engineer", "startDate": "Jan 2020" } ], "hobbies": ["chess"] }';

test('resume-to-json extracts a resume into JSON Resume fields (auto mode, pretty default)', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  await page.fill('#in-data', SAMPLE);
  const out = page.locator('#tool-output');
  // pretty defaults ON -> indented `"key": value` shapes.
  await expect(out).toContainText('"name": "Jane Doe"', { timeout: 15000 });
  await expect(out).toContainText('"label": "Senior Software Engineer"');
  await expect(out).toContainText('"email": "jane.doe@example.com"');
  await expect(out).toContainText('"city": "San Francisco"');
  await expect(out).toContainText('"network": "LinkedIn"');
  await expect(out).toContainText('"username": "janedoe"');
  // Work entry: dates normalized to ISO-8601 partials; Present -> no endDate in that entry.
  await expect(out).toContainText('"name": "Acme Corp"');
  await expect(out).toContainText('"startDate": "2020-01"');
  await expect(out).toContainText('"endDate": "2019-12"'); // Beta Labs closed range
  await expect(out).toContainText('Led migration to a service mesh across 40 services');
  // Education split: degree -> studyType + area, GPA -> score.
  await expect(out).toContainText('"institution": "State University"');
  await expect(out).toContainText('"studyType": "B.S."');
  await expect(out).toContainText('"area": "Computer Science"');
  await expect(out).toContainText('"score": "3.8/4.0"');
  // Grouped skills + languages with fluency.
  await expect(out).toContainText('"keywords"');
  await expect(out).toContainText('"language": "English"');
  await expect(out).toContainText('"fluency": "Native"');
});

test('resume-to-json validate mode reports errors, warnings and a summary', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  await page.fill('#in-data', INVALID_DOC);
  await page.selectOption('#in-mode', 'validate');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15000 });
  await expect(out).toContainText('work[0].startDate');
  await expect(out).toContainText('is not an ISO-8601 date');
  await expect(out).toContainText('hobbies: unknown top-level section');
  await expect(out).toContainText('"counts"');
});

test('resume-to-json extract mode honours schema_ref on and pretty off (non-default checkboxes)', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  await page.fill('#in-data', 'Alan Turing\nMathematician\nalan@example.com\n\nSkills\nMathematics, Cryptanalysis');
  await page.selectOption('#in-mode', 'extract');
  await page.check('#in-schema_ref');
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  // pretty OFF -> compact one-line JSON (no space after the colon).
  await expect(out).toContainText(
    '"$schema":"https://raw.githubusercontent.com/jsonresume/resume-schema/v1.0.0/schema.json"',
    { timeout: 15000 }
  );
  await expect(out).toContainText('"meta":{"version":"v1.0.0"}');
  await expect(out).toContainText('"name":"Alan Turing"');
  await expect(out).toContainText('{"name":"Mathematics"}');
});

test('resume-to-json auto mode routes a JSON object to validation', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  await page.fill('#in-data', '{"basics":{"name":"A","email":"a@b.co"},"work":[{"name":"X","startDate":"2020-01"}]}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": true', { timeout: 15000 });
  await expect(out).toContainText('"errors": []');
});

test('resume-to-json enforces the 1 MiB cap exactly (at passes, one over errors)', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  const MAX = 1048576;
  // Exactly at the cap: extraction succeeds.
  await page.evaluate((max) => {
    const el = document.querySelector('#in-data') as HTMLTextAreaElement;
    const head = 'Jane Doe\n';
    el.value = head + 'x'.repeat(max - head.length);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, MAX);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "Jane Doe"', { timeout: 20000 });
  // One byte over: rejected with a clear error.
  await page.evaluate((max) => {
    const el = document.querySelector('#in-data') as HTMLTextAreaElement;
    const head = 'Jane Doe\n';
    el.value = head + 'x'.repeat(max - head.length + 1);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, MAX);
  await expect(out).toContainText('input is too large', { timeout: 20000 });
  await expect(out).toContainText('1048576');
});

test('resume-to-json shows a graceful error for validate mode on plain text', async ({ page }) => {
  await page.goto('/tools/resume-to-json/');
  await page.fill('#in-data', 'just some plain text');
  await page.selectOption('#in-mode', 'validate');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('expects a resume.json document', { timeout: 15000 });
});

test('resume-to-json pre-fills from query params (deep link)', async ({ page }) => {
  await page.goto(
    '/tools/resume-to-json/?data=' + encodeURIComponent(INVALID_DOC) + '&mode=validate&pretty=false'
  );
  await expect(page.locator('#in-data')).toHaveValue(INVALID_DOC);
  await expect(page.locator('#in-mode')).toHaveValue('validate');
  const out = page.locator('#tool-output');
  // pretty=false from the query -> compact report.
  await expect(out).toContainText('"valid":false', { timeout: 15000 });
  await expect(out).toContainText('work[0].startDate');
});
