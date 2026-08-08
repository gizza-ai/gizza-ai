import { test, expect } from './fixtures';

const people = `dn: uid=ada,ou=people,dc=example,dc=com
objectClass: top
objectClass: person
cn: Ada Lovelace
mail: ada@example.com

dn: uid=bo,ou=people,dc=example,dc=com
objectClass: top
cn: Bo Diaz
mail: bo@example.com`;

const output = (page) => page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('ldif-to-csv page converts entries with joined repeated attributes', async ({ page }) => {
  await page.goto('/tools/ldif-to-csv/');
  await page.fill('#in-ldif', people);

  await expect(page.locator('#tool-output')).toContainText('objectClass', { timeout: 15000 });
  expect(await output(page)).toBe(
    'dn,objectClass,cn,mail\n' +
      '"uid=ada,ou=people,dc=example,dc=com",top|person,Ada Lovelace,ada@example.com\n' +
      '"uid=bo,ou=people,dc=example,dc=com",top,Bo Diaz,bo@example.com',
  );
});

test('ldif-to-csv deep-link applies indexed repeated attributes and selected columns', async ({ page }) => {
  const ldif = `dn: cn=Group,dc=example,dc=com
member: uid=ada,dc=example,dc=com
member: uid=bo,dc=example,dc=com
member: uid=cy,dc=example,dc=com
cn: Group`;
  const qs = new URLSearchParams({
    ldif,
    columns: 'cn,member',
    include_dn: 'true',
    multi_value: 'indexed',
    value_separator: '|',
    max_indexed: '2',
    decode_base64: 'true',
    delimiter: ',',
    include_changetype: 'false',
  });
  await page.goto(`/tools/ldif-to-csv/?${qs.toString()}`);

  await expect(page.locator('#in-multi_value')).toHaveValue('indexed');
  await expect(page.locator('#in-max_indexed')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toContainText('member.2', { timeout: 15000 });
  expect(await output(page)).toBe(
    'dn,cn,member,member.2\n' +
      '"cn=Group,dc=example,dc=com",Group,"uid=ada,dc=example,dc=com","uid=bo,dc=example,dc=com|uid=cy,dc=example,dc=com"',
  );
});

test('ldif-to-csv page decodes base64 text and supports changetype/tab output', async ({ page }) => {
  await page.goto('/tools/ldif-to-csv/');
  await page.fill(
    '#in-ldif',
    'version: 1\n' +
      'dn:: Y249Q2Fmw6ksZGM9ZXhhbXBsZSxkYz1jb20=\n' +
      'changetype: modify\n' +
      'replace: cn\n' +
      'cn:: Q2Fmw6k=\n' +
      '-\n',
  );
  await page.fill('#in-columns', 'cn');
  await page.check('#in-include_changetype');
  await page.fill('#in-delimiter', 'tab');

  await expect(page.locator('#tool-output')).toContainText('Café', { timeout: 15000 });
  expect(await output(page)).toBe('dn\tchangetype\tcn\ncn=Café,dc=example,dc=com\tmodify\tCafé');
});
