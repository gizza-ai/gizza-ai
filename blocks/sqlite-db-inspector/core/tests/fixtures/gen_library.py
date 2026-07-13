#!/usr/bin/env python3
"""Generate the deterministic SQLite fixture used by the core unit tests.

A small "library" schema that exercises every feature the inspector reports
(creation order = sqlite_master order):

  authors  — rowid table; INTEGER PRIMARY KEY alias, NOT NULL column
  books    — rowid table; inline FOREIGN KEY … REFERENCES authors ON DELETE
             CASCADE, NOT NULL, DEFAULT, a REAL column
  reviews  — rowid table (implicit rowid); a *table-level* FOREIGN KEY clause
  book_titles (VIEW) — a saved query over books ⨝ authors
  settings — a WITHOUT ROWID table (row-count is unsupported → the inspector
             must say so rather than guess)

Plus two explicit indexes (one UNIQUE) on `books` so index reporting is tested.
"""
import os
import sqlite3

here = os.path.dirname(os.path.abspath(__file__))
path = os.path.join(here, "library.db")
if os.path.exists(path):
    os.remove(path)

con = sqlite3.connect(path)
con.execute("PRAGMA page_size=4096")
con.execute("PRAGMA foreign_keys=ON")
cur = con.cursor()

cur.execute(
    "CREATE TABLE authors ("
    " id INTEGER PRIMARY KEY,"
    " name TEXT NOT NULL,"
    " country TEXT"
    ")"
)
cur.executemany(
    "INSERT INTO authors VALUES (?,?,?)",
    [
        (1, "Ada Lovelace", "UK"),
        (2, "Grace Hopper", "US"),
        (3, "Alan Turing", None),
    ],
)

cur.execute(
    "CREATE TABLE books ("
    " id INTEGER PRIMARY KEY,"
    " title TEXT NOT NULL,"
    " author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,"
    " year INTEGER,"
    " price REAL DEFAULT 0.0"
    ")"
)
cur.executemany(
    "INSERT INTO books VALUES (?,?,?,?,?)",
    [
        (1, "Notes on the Analytical Engine", 1, 1843, 12.5),
        (2, "The Compiler", 2, 1952, 30.0),
        (3, "On Computable Numbers", 3, 1936, 0.0),
        (4, "Cobol Stories", 2, 1960, 9.99),
    ],
)

cur.execute("CREATE UNIQUE INDEX idx_books_title ON books(title)")
cur.execute("CREATE INDEX idx_books_author ON books(author_id, year)")

# reviews: no INTEGER PRIMARY KEY → implicit rowid table; table-level FK.
cur.execute(
    "CREATE TABLE reviews ("
    " book_id INTEGER,"
    " reviewer TEXT,"
    " rating INTEGER,"
    " FOREIGN KEY (book_id) REFERENCES books(id)"
    ")"
)
cur.executemany(
    "INSERT INTO reviews VALUES (?,?,?)",
    [(1, "reader1", 5), (2, "reader2", 4)],
)

cur.execute(
    "CREATE VIEW book_titles AS "
    "SELECT b.title AS title, a.name AS author "
    "FROM books b JOIN authors a ON a.id = b.author_id"
)

# WITHOUT ROWID table: row counts are unsupported and must be flagged.
cur.execute(
    "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT) WITHOUT ROWID"
)
cur.executemany(
    "INSERT INTO settings VALUES (?,?)",
    [("theme", "dark"), ("lang", "en")],
)

con.commit()
con.execute("VACUUM")
con.close()
print("wrote", path, os.path.getsize(path), "bytes")
