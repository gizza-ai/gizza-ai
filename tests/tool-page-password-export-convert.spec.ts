import { test, expect } from './fixtures';

// Every credential below is invented and belongs to no real account.
const KEEPASS = `Group,Title,Username,Password,URL,Notes,TOTP
Work,Example Mail,demo-user@example.com,sample-passphrase-1,https://mail.example.com,recovery codes in the safe,JBSWY3DPEHPK3PXP`;
const CHROME = `name,url,username,password,note
Example Shop,https://shop.example.com,demo-user@example.com,sample-passphrase-2,gift card inside`;
const BITWARDEN_CSV = `folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp
Work,0,login,Example Mail,recovery codes in the safe,,0,https://mail.example.com,demo-user@example.com,sample-passphrase-1,JBSWY3DPEHPK3PXP`;
const LASTPASS = `url,username,password,totp,extra,name,grouping,fav
https://mail.example.com,demo-user@example.com,sample-passphrase-1,JBSWY3DPEHPK3PXP,recovery codes in the safe,Example Mail,Work,0`;
const GENERIC = `folder,name,url,username,password,notes,totp,favorite,type
Work,Example Mail,https://mail.example.com,demo-user@example.com,sample-passphrase-1,recovery codes in the safe,JBSWY3DPEHPK3PXP,0,login`;

async function runWasm(
  page: any,
  data: string = KEEPASS,
  from = 'auto',
  to = 'keepass-csv',
  includeTotp = 'true',
  includeExtraFields = 'true',
  skipEmptyPasswords = 'false',
  defaultFolder = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/password-export-convert/gizza_ai_password_export_convert_web.js');
    await mod.default('/tools/password-export-convert/gizza_ai_password_export_convert_web_bg.wasm');
    return mod.run(
      args.data,
      args.from,
      args.to,
      args.includeTotp,
      args.includeExtraFields,
      args.skipEmptyPasswords,
      args.defaultFolder,
    );
  }, { data, from, to, includeTotp, includeExtraFields, skipEmptyPasswords, defaultFolder });
}

test('password-export-convert page converts fake KeePass CSV to Bitwarden JSON', async ({ page }) => {
  await page.goto('/tools/password-export-convert/');
  await page.fill('#in-data', KEEPASS);
  await page.selectOption('#in-to', 'bitwarden-json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"encrypted": false', { timeout: 20_000 });
  await expect(output).toContainText('"name": "Example Mail"');
  await expect(output).toContainText('"username": "demo-user@example.com"');
  await expect(output).toContainText('JBSWY3DPEHPK3PXP');
});

test('password-export-convert deep link covers Chrome CSV, skip empty and default folder', async ({ page }) => {
  const params = new URLSearchParams({
    data: `${CHROME}\nBlank,https://blank.example.com,user@example.com,,no password`,
    from: 'chrome-csv',
    to: 'bitwarden-csv',
    include_totp: 'false',
    include_extra_fields: 'true',
    skip_empty_passwords: 'true',
    default_folder: 'Imported',
  });
  await page.goto(`/tools/password-export-convert/?${params.toString()}`);

  await expect(page.locator('#in-from')).toHaveValue('chrome-csv', { timeout: 15_000 });
  await expect(page.locator('#in-to')).toHaveValue('bitwarden-csv');
  // Non-default checkbox states arrive from the query string.
  await expect(page.locator('#in-skip_empty_passwords')).toBeChecked();
  await expect(page.locator('#in-include_totp')).not.toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('Example Shop', { timeout: 20_000 });
  await expect(output).toContainText('Imported');
  await expect(output).not.toContainText('Blank');
});

test('password-export-convert writes every target format exactly', async ({ page }) => {
  await page.goto('/tools/password-export-convert/');

  expect((await runWasm(page, KEEPASS, 'auto', 'keepass-csv')).trimEnd()).toBe(
    `"Group","Title","Username","Password","URL","Notes","TOTP","Icon","Last Modified","Created"
"Work","Example Mail","demo-user@example.com","sample-passphrase-1","https://mail.example.com","recovery codes in the safe","JBSWY3DPEHPK3PXP","0","",""`,
  );

  expect((await runWasm(page, KEEPASS, 'auto', 'bitwarden-csv')).trimEnd()).toBe(BITWARDEN_CSV);

  expect((await runWasm(page, KEEPASS, 'auto', 'lastpass-csv')).trimEnd()).toBe(LASTPASS);

  expect((await runWasm(page, KEEPASS, 'auto', 'chrome-csv')).trimEnd()).toBe(
    `name,url,username,password,note
Example Mail,https://mail.example.com,demo-user@example.com,sample-passphrase-1,"recovery codes in the safe
TOTP: JBSWY3DPEHPK3PXP
Folder: Work"`,
  );

  expect((await runWasm(page, KEEPASS, 'auto', 'generic-csv')).trimEnd()).toBe(GENERIC);

  const json = await runWasm(page, KEEPASS, 'auto', 'bitwarden-json');
  expect(json).toContain('"encrypted": false');
  expect(json).toContain('"name": "Work"'); // the folder survived as a Bitwarden folder
  expect(json).toContain('"totp": "JBSWY3DPEHPK3PXP"');
  // Ids are hashed from the entry text, so the same export converts to the same bytes.
  expect(await runWasm(page, KEEPASS, 'auto', 'bitwarden-json')).toBe(json);
});

test('password-export-convert reads every source format and honours the options', async ({ page }) => {
  await page.goto('/tools/password-export-convert/');

  // Each explicit `from` value reaches the same neutral entry, so each converts to the same sheet.
  for (const [data, from] of [
    [KEEPASS, 'keepass-csv'],
    [BITWARDEN_CSV, 'bitwarden-csv'],
    [LASTPASS, 'lastpass-csv'],
    [GENERIC, 'generic-csv'],
  ] as const) {
    expect((await runWasm(page, data, from, 'generic-csv')).trimEnd()).toBe(GENERIC);
  }
  expect((await runWasm(page, CHROME, 'chrome-csv', 'generic-csv')).trimEnd()).toBe(
    `folder,name,url,username,password,notes,totp,favorite,type
,Example Shop,https://shop.example.com,demo-user@example.com,sample-passphrase-2,gift card inside,,0,login`,
  );
  const fromJson = await runWasm(
    page,
    '{"encrypted":false,"folders":[{"id":"f1","name":"Work"}],"items":[{"id":"i1","folderId":"f1","type":1,"name":"Example Mail","notes":"recovery codes in the safe","favorite":false,"login":{"uris":[{"uri":"https://mail.example.com"}],"username":"demo-user@example.com","password":"sample-passphrase-1","totp":"JBSWY3DPEHPK3PXP"}}]}',
    'bitwarden-json',
    'generic-csv',
  );
  expect(fromJson.trimEnd()).toBe(GENERIC);

  // include_totp off strips the 2FA secret; include_extra_fields off drops unclaimed columns.
  const withExtras = `${KEEPASS.replace('TOTP\n', 'TOTP,Security question\n')},first pet`;
  const stripped = await runWasm(page, withExtras, 'auto', 'generic-csv', 'false', 'false');
  expect(stripped).toContain('folder,name,url,username,password,notes,totp,favorite,type');
  expect(stripped).not.toContain('JBSWY3DPEHPK3PXP');
  expect(stripped).not.toContain('first pet');
  expect(await runWasm(page, withExtras, 'auto', 'generic-csv', 'true', 'true'))
    .toContain('Security question: first pet');

  // Bookkeeping columns are dropped rather than pasted into every note.
  const fullKeepass = await runWasm(page, KEEPASS, 'auto', 'keepass-csv');
  expect(await runWasm(page, fullKeepass, 'auto', 'generic-csv')).not.toContain('Icon');

  // default_folder only fills entries that have none.
  expect(await runWasm(page, CHROME, 'auto', 'generic-csv', 'true', 'true', 'false', 'Imported')).toContain('Imported,Example Shop');
});

test('password-export-convert reports bad input and ships a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/password-export-convert/');

  await expect(runWasm(page, '{"encrypted":true,"data":"2.abc|def"}', 'bitwarden-json', 'keepass-csv'))
    .rejects.toThrow(/encrypted/);
  await expect(runWasm(page, 'first,last,city\nA,Person,Berlin', 'auto', 'keepass-csv'))
    .rejects.toThrow(/could not tell which export format/);
  await expect(runWasm(page, '   ', 'auto', 'keepass-csv')).rejects.toThrow(/empty/);
  await expect(runWasm(page, KEEPASS, 'auto', '1password-csv')).rejects.toThrow(/unknown format/);
  // The 5000-entry cap, at its exact boundary.
  const big = (n: number) =>
    'name,username,password\n' + Array.from({ length: n }, (_, i) => `Site ${i},demo-user,sample-passphrase-${i}`).join('\n') + '\n';
  expect(await runWasm(page, big(5000), 'auto', 'generic-csv')).toContain('Site 4999');
  await expect(runWasm(page, big(5001), 'auto', 'generic-csv')).rejects.toThrow(/5000 entries/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool password-export-convert');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
