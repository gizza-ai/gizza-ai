#!/usr/bin/env python3
"""Generate the browser-history-parser test fixtures: a Chrome/Edge `History`
database and a Firefox `places.sqlite` database, each with a handful of rows.
Run from this directory:  python3 gen_fixtures.py

Chrome/Edge store visit times as microseconds since 1601-01-01 UTC (WebKit epoch);
Firefox stores them as microseconds since 1970-01-01 UTC (PRTime). We compute the
raw values here so the Rust tests can assert exact readable timestamps.
"""
import os
import sqlite3
from datetime import datetime, timezone

WEBKIT_OFFSET_SECONDS = 11644473600  # seconds between 1601-01-01 and 1970-01-01


def unix(y, mo, d, h, mi, s):
    return int(datetime(y, mo, d, h, mi, s, tzinfo=timezone.utc).timestamp())


def webkit(unix_s):
    return (unix_s + WEBKIT_OFFSET_SECONDS) * 1_000_000


def prtime(unix_s):
    return unix_s * 1_000_000


def build_chrome(path):
    if os.path.exists(path):
        os.remove(path)
    db = sqlite3.connect(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE urls (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, "
        "visit_count INTEGER DEFAULT 0, typed_count INTEGER DEFAULT 0, "
        "last_visit_time INTEGER, hidden INTEGER DEFAULT 0)"
    )
    c.execute(
        "CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, "
        "from_visit INTEGER, transition INTEGER DEFAULT 0, segment_id INTEGER, "
        "visit_duration INTEGER DEFAULT 0)"
    )
    urls = [
        (1, "https://www.rust-lang.org/", "Rust Programming Language", 5, 3),
        (2, "https://news.example.com/article", "Breaking News", 2, 2),
    ]
    for uid, url, title, vc, tc in urls:
        c.execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time) "
            "VALUES (?,?,?,?,?,?)",
            (uid, url, title, vc, tc, webkit(unix(2024, 2, 1, 12, 0, 0))),
        )
    # (url_id, unix_time, transition-with-qualifier-bits)
    visits = [
        (1, unix(2024, 1, 15, 10, 30, 0), 0x00000000),  # LINK
        (2, unix(2023, 12, 25, 8, 0, 0), 0x30000001),   # TYPED + chain qualifiers
        (1, unix(2024, 2, 1, 12, 0, 0), 0x00000008),    # RELOAD
    ]
    for i, (uid, ut, trans) in enumerate(visits, start=1):
        c.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (?,?,?,?,?)",
            (i, uid, webkit(ut), 0, trans),
        )
    db.commit()
    db.close()


def build_firefox(path):
    if os.path.exists(path):
        os.remove(path)
    db = sqlite3.connect(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, "
        "rev_host LONGVARCHAR, visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0, "
        "typed INTEGER DEFAULT 0, last_visit_date INTEGER)"
    )
    c.execute(
        "CREATE TABLE moz_historyvisits (id INTEGER PRIMARY KEY, from_visit INTEGER, "
        "place_id INTEGER, visit_date INTEGER, visit_type INTEGER, session INTEGER DEFAULT 0)"
    )
    places = [
        (1, "https://www.mozilla.org/", "Mozilla", 3),
        (2, "https://blog.example.org/post", "A Blog Post", 1),
    ]
    for pid, url, title, vc in places:
        c.execute(
            "INSERT INTO moz_places (id, url, title, visit_count, last_visit_date) VALUES (?,?,?,?,?)",
            (pid, url, title, vc, prtime(unix(2024, 3, 11, 18, 45, 0))),
        )
    visits = [
        (1, unix(2024, 3, 10, 9, 15, 0), 1),   # LINK
        (2, unix(2022, 6, 1, 0, 0, 0), 2),     # TYPED
        (1, unix(2024, 3, 11, 18, 45, 0), 3),  # BOOKMARK
    ]
    for i, (pid, ut, vtype) in enumerate(visits, start=1):
        c.execute(
            "INSERT INTO moz_historyvisits (id, from_visit, place_id, visit_date, visit_type) "
            "VALUES (?,?,?,?,?)",
            (i, 0, pid, prtime(ut), vtype),
        )
    db.commit()
    db.close()


def build_other(path):
    """A valid SQLite DB that is NOT a browser history (for the error path)."""
    if os.path.exists(path):
        os.remove(path)
    db = sqlite3.connect(path)
    db.execute("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)")
    db.execute("INSERT INTO notes (body) VALUES ('hello')")
    db.commit()
    db.close()


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build_chrome(os.path.join(here, "chrome_history.db"))
    build_firefox(os.path.join(here, "firefox_places.sqlite"))
    build_other(os.path.join(here, "other.db"))
    # Print the raw + readable timestamps the Rust tests will assert.
    print("WEBKIT_OFFSET_SECONDS =", WEBKIT_OFFSET_SECONDS)
    for label, y, mo, d, h, mi, s in [
        ("chrome visit A LINK", 2024, 1, 15, 10, 30, 0),
        ("chrome visit B TYPED", 2023, 12, 25, 8, 0, 0),
        ("chrome visit C RELOAD", 2024, 2, 1, 12, 0, 0),
        ("firefox visit 1 LINK", 2024, 3, 10, 9, 15, 0),
        ("firefox visit 2 TYPED", 2022, 6, 1, 0, 0, 0),
        ("firefox visit 3 BOOKMARK", 2024, 3, 11, 18, 45, 0),
    ]:
        u = unix(y, mo, d, h, mi, s)
        iso = datetime.fromtimestamp(u, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        print(f"{label:26} unix={u:>12}  webkit={webkit(u):>20}  prtime={prtime(u):>18}  iso={iso}")


if __name__ == "__main__":
    main()
