import { test, expect } from './fixtures';

const blogSchema = `datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model User {
  id    Int     @id @default(autoincrement())
  email String  @unique
  posts Post[]
}

model Post {
  id       Int  @id @default(autoincrement())
  title    String
  author   User @relation(fields: [authorId], references: [id], onDelete: Cascade)
  authorId Int

  @@index([authorId])
}`;

test('prisma-schema-to-sql page emits exact PostgreSQL DDL with relation and indexes', async ({ page }) => {
  await page.goto('/tools/prisma-schema-to-sql/');
  await page.fill('#in-input', blogSchema);
  await page.selectOption('#in-dialect', 'postgresql');

  await expect(page.locator('#tool-output')).toHaveText(
    `CREATE TABLE "User" (
    "id" SERIAL NOT NULL,
    "email" TEXT NOT NULL,
    CONSTRAINT "User_pkey" PRIMARY KEY ("id")
);

CREATE TABLE "Post" (
    "id" SERIAL NOT NULL,
    "title" TEXT NOT NULL,
    "authorId" INTEGER NOT NULL,
    CONSTRAINT "Post_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "User_email_key" ON "User"("email");
CREATE INDEX "Post_authorId_idx" ON "Post"("authorId");

ALTER TABLE "Post" ADD CONSTRAINT "Post_authorId_fkey" FOREIGN KEY ("authorId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;`,
    { timeout: 15000 },
  );
});

test('prisma-schema-to-sql honours deep link dialect and toggles', async ({ page }) => {
  const schema = `model Task {
  id    Int     @id @default(autoincrement())
  title String
  done  Boolean @default(false)
}`;
  const qs =
    '?input=' + encodeURIComponent(schema) +
    '&dialect=sqlite' +
    '&foreign_keys=false' +
    '&indexes=false' +
    '&if_not_exists=true' +
    '&drop_if_exists=true' +
    '&quote_identifiers=false';
  await page.goto('/tools/prisma-schema-to-sql/' + qs);

  await expect(page.locator('#in-dialect')).toHaveValue('sqlite', { timeout: 15000 });
  await expect(page.locator('#in-foreign_keys')).not.toBeChecked();
  await expect(page.locator('#in-indexes')).not.toBeChecked();
  await expect(page.locator('#in-if_not_exists')).toBeChecked();
  await expect(page.locator('#in-drop_if_exists')).toBeChecked();
  await expect(page.locator('#in-quote_identifiers')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText(
    `DROP TABLE IF EXISTS Task;

CREATE TABLE IF NOT EXISTS Task (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    done BOOLEAN NOT NULL DEFAULT false
);`,
    { timeout: 15000 },
  );
});
