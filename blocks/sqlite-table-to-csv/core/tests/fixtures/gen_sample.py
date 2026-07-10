#!/usr/bin/env python3
"""Generate the deterministic SQLite fixture used by the core unit tests.

Tables (creation order = sqlite_master order):
  people  — rowid alias + REAL/BLOB/NULL + CSV-quoting + unicode
  empties — one column, zero rows
  big     — one huge TEXT cell → forces record overflow pages
  many    — 2000 rows → forces a multi-leaf b-tree with an interior page
"""
import os
import sqlite3

here = os.path.dirname(os.path.abspath(__file__))
path = os.path.join(here, "sample.db")
if os.path.exists(path):
    os.remove(path)

con = sqlite3.connect(path)
con.execute("PRAGMA page_size=4096")
cur = con.cursor()

cur.execute(
    "CREATE TABLE people (id INTEGER PRIMARY KEY, name TEXT, score REAL, bio TEXT, avatar BLOB, notes TEXT)"
)
cur.execute(
    "INSERT INTO people VALUES (?,?,?,?,?,?)",
    (1, "Ada", 9.5, "Coder, poet", sqlite3.Binary(bytes([0x00, 0xFF, 0x10])), None),
)
cur.execute(
    "INSERT INTO people VALUES (?,?,?,?,?,?)",
    (2, 'Bob "the builder"', 7.0, None, None, "plain"),
)
cur.execute(
    "INSERT INTO people VALUES (?,?,?,?,?,?)",
    (3, "Zoë", 42.0, None, None, "unicode ✓"),
)

cur.execute("CREATE TABLE empties (label TEXT)")

cur.execute("CREATE TABLE big (id INTEGER PRIMARY KEY, blob TEXT)")
cur.execute("INSERT INTO big VALUES (?,?)", (1, "x" * 5000))

cur.execute("CREATE TABLE many (id INTEGER PRIMARY KEY, n INTEGER)")
cur.executemany(
    "INSERT INTO many VALUES (?,?)", [(i, i * 10) for i in range(1, 2001)]
)

con.commit()
con.execute("VACUUM")
con.close()
print("wrote", path, os.path.getsize(path), "bytes")
