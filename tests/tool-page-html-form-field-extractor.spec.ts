import { test, expect } from './fixtures';

const signupHtml = '<form id="signup" action="/register" method="post">\n' +
  '  <label for="email">Email address</label>\n' +
  '  <input type="email" id="email" name="email" required placeholder="you@example.com" maxlength="64">\n' +
  '  <label>Age <input type="number" name="age" min="18" max="120" value="18"></label>\n' +
  '  <input type="hidden" name="csrf" value="tok123">\n' +
  '  <button type="submit" name="action" value="signup">Sign up</button>\n' +
  '</form>';

test('html-form-field-extractor page lists form fields as markdown', async ({ page }) => {
  await page.goto('/tools/html-form-field-extractor/');
  await page.fill('#in-html', signupHtml);
  await page.selectOption('#in-format', 'markdown');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## Form 0 — POST /register', { timeout: 15_000 });
  await expect(out).toContainText('| 1 | `email` | `email` | Email address | yes | — | maxlength=`64` |');
  await expect(out).toContainText('| 2 | `age` | `number` | Age | no | `18` | min=`18`, max=`120` |');
  await expect(out).toContainText('| 3 | `csrf` | `hidden` | — | no | `tok123` | — |');
  await expect(out).not.toContainText('| 4 | `action` | `submit` | Sign up | no | `signup` | — |');
});

test('html-form-field-extractor deep-link renders csv with non-default buttons', async ({ page }) => {
  const qs = new URLSearchParams({
    html: signupHtml,
    format: 'csv',
    form_index: '0',
    include_buttons: 'true',
    include_hidden: 'true',
    include_labels: 'true',
  });
  await page.goto(`/tools/html-form-field-extractor/?${qs.toString()}`);

  await expect(page.locator('#in-html')).toHaveValue(signupHtml, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-form_index')).toHaveValue('0');
  await expect(page.locator('#in-include_buttons')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('form_index,form_action,form_method,tag,type,name,id,label,required,default');
  await expect(out).toContainText('0,/register,post,input,email,email,email,Email address,true');
  await expect(out).toContainText('0,/register,post,button,submit,action,,Sign up,false,signup');
});
