#!/usr/bin/env python3
"""Generate the browser-artifact-parser test fixtures — one small SQLite file
per supported browser artifact type, each with a handful of rows. Run from this
directory:  python3 gen_fixtures.py

Timestamp epochs (all normalized to UTC unix seconds by the Rust parser):
  - Chrome/Chromium (History `visits`, `downloads`; `Cookies`): WebKit
    microseconds since 1601-01-01 UTC.
  - Firefox (`moz_historyvisits`, `moz_cookies`, legacy `moz_downloads`):
    PRTime microseconds since 1970-01-01 UTC (Firefox cookie `expiry` is
    SECONDS since 1970 — not asserted for the timeline).
  - Safari (`History.db`): CFAbsoluteTime = REAL seconds since 2001-01-01 UTC.
  - Safari (`Cache.db` `cfurl_cache_response.time_stamp`): a TEXT UTC datetime
    "YYYY-MM-DD HH:MM:SS".

We compute the raw values here so the Rust tests can assert exact readable
timestamps.
"""
import os
import sqlite3
from datetime import datetime, timezone

WEBKIT_OFFSET_SECONDS = 11644473600     # 1601-01-01 -> 1970-01-01
COCOA_OFFSET_SECONDS = 978307200        # 2001-01-01 -> 1970-01-01 (CFAbsoluteTime)


def unix(y, mo, d, h, mi, s):
    return int(datetime(y, mo, d, h, mi, s, tzinfo=timezone.utc).timestamp())


def webkit(unix_s):
    return (unix_s + WEBKIT_OFFSET_SECONDS) * 1_000_000


def prtime(unix_s):
    return unix_s * 1_000_000


def cocoa(unix_s):
    return float(unix_s - COCOA_OFFSET_SECONDS)


def iso(unix_s):
    return datetime.fromtimestamp(unix_s, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def fresh(path):
    if os.path.exists(path):
        os.remove(path)
    return sqlite3.connect(path)


def build_chrome_history(path):
    """Chrome/Edge `History`: urls + visits + downloads(+chains)."""
    db = fresh(path)
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
    c.execute(
        "CREATE TABLE downloads (id INTEGER PRIMARY KEY, guid VARCHAR, current_path LONGVARCHAR, "
        "target_path LONGVARCHAR, start_time INTEGER, received_bytes INTEGER, total_bytes INTEGER, "
        "state INTEGER, tab_url LONGVARCHAR, mime_type VARCHAR)"
    )
    c.execute(
        "CREATE TABLE downloads_url_chains (id INTEGER, chain_index INTEGER, url LONGVARCHAR, "
        "PRIMARY KEY (id, chain_index))"
    )
    urls = [
        (1, "https://www.rust-lang.org/", "Rust Programming Language", 5),
        (2, "https://news.example.com/article", "Breaking News", 2),
    ]
    for uid, url, title, vc in urls:
        c.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (?,?,?,?,?)",
            (uid, url, title, vc, webkit(unix(2024, 2, 1, 12, 0, 0))),
        )
    visits = [
        (1, unix(2024, 1, 15, 10, 30, 0), 0x00000000),  # LINK
        (2, unix(2023, 12, 25, 8, 0, 0), 0x00000001),   # TYPED
    ]
    for i, (uid, ut, trans) in enumerate(visits, start=1):
        c.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (?,?,?,?,?)",
            (i, uid, webkit(ut), 0, trans),
        )
    # One download; its final URL lives in downloads_url_chains (highest index).
    c.execute(
        "INSERT INTO downloads (id, target_path, start_time, received_bytes, total_bytes, "
        "state, tab_url, mime_type) VALUES (?,?,?,?,?,?,?,?)",
        (1, "/home/u/Downloads/rustup-init", webkit(unix(2024, 1, 20, 9, 5, 0)),
         5_242_880, 5_242_880, 1, "https://rustup.rs/", "application/octet-stream"),
    )
    c.execute("INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (?,?,?)",
              (1, 0, "https://redirect.example.com/rustup-init"))
    c.execute("INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (?,?,?)",
              (1, 1, "https://static.rust-lang.org/rustup/rustup-init"))
    db.commit()
    db.close()


def build_firefox_places(path):
    """Firefox `places.sqlite`: moz_places + moz_historyvisits."""
    db = fresh(path)
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
    ]
    for i, (pid, ut, vtype) in enumerate(visits, start=1):
        c.execute(
            "INSERT INTO moz_historyvisits (id, from_visit, place_id, visit_date, visit_type) "
            "VALUES (?,?,?,?,?)",
            (i, 0, pid, prtime(ut), vtype),
        )
    db.commit()
    db.close()


def build_chrome_cookies(path):
    """Chrome/Edge `Cookies`: cookies table (creation_utc = WebKit us)."""
    db = fresh(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE cookies (creation_utc INTEGER, host_key TEXT, name TEXT, value TEXT, "
        "path TEXT, expires_utc INTEGER, is_secure INTEGER, is_httponly INTEGER, "
        "last_access_utc INTEGER)"
    )
    rows = [
        (webkit(unix(2024, 5, 1, 8, 0, 0)), ".github.com", "logged_in", "yes", "/",
         webkit(unix(2025, 5, 1, 8, 0, 0)), 1, 1, webkit(unix(2024, 5, 2, 9, 0, 0))),
        (webkit(unix(2023, 11, 20, 14, 30, 0)), "example.com", "sessionid", "abc123", "/",
         webkit(unix(2024, 11, 20, 14, 30, 0)), 1, 0, webkit(unix(2023, 11, 21, 0, 0, 0))),
    ]
    for r in rows:
        c.execute(
            "INSERT INTO cookies (creation_utc, host_key, name, value, path, expires_utc, "
            "is_secure, is_httponly, last_access_utc) VALUES (?,?,?,?,?,?,?,?,?)", r)
    db.commit()
    db.close()


def build_firefox_cookies(path):
    """Firefox `cookies.sqlite`: moz_cookies (creationTime = PRTime us)."""
    db = fresh(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, originAttributes TEXT, name TEXT, "
        "value TEXT, host TEXT, path TEXT, expiry INTEGER, lastAccessed INTEGER, "
        "creationTime INTEGER, isSecure INTEGER, isHttpOnly INTEGER)"
    )
    rows = [
        (1, "pref", "yes", ".mozilla.org", "/", unix(2025, 1, 1, 0, 0, 0),
         prtime(unix(2024, 4, 15, 7, 0, 0)), prtime(unix(2024, 4, 10, 6, 0, 0))),
        (2, "sid", "xyz789", "wiki.example.net", "/", unix(2024, 8, 1, 0, 0, 0),
         prtime(unix(2024, 1, 5, 12, 0, 0)), prtime(unix(2024, 1, 5, 11, 0, 0))),
    ]
    for r in rows:
        c.execute(
            "INSERT INTO moz_cookies (id, name, value, host, path, expiry, lastAccessed, "
            "creationTime) VALUES (?,?,?,?,?,?,?,?)", r)
    db.commit()
    db.close()


def build_safari_history(path):
    """Safari `History.db`: history_items + history_visits (CFAbsoluteTime)."""
    db = fresh(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE history_items (id INTEGER PRIMARY KEY, url TEXT, domain_expansion TEXT, "
        "visit_count INTEGER, daily_visit_counts BLOB)"
    )
    c.execute(
        "CREATE TABLE history_visits (id INTEGER PRIMARY KEY, history_item INTEGER, "
        "visit_time REAL, title TEXT, load_successful INTEGER DEFAULT 1, http_non_get INTEGER, "
        "origin INTEGER)"
    )
    items = [
        (1, "https://www.apple.com/", "apple", 4),
        (2, "https://developer.example.com/docs", "example", 1),
    ]
    for iid, url, dom, vc in items:
        c.execute(
            "INSERT INTO history_items (id, url, domain_expansion, visit_count) VALUES (?,?,?,?)",
            (iid, url, dom, vc))
    visits = [
        (1, 1, unix(2024, 6, 2, 16, 20, 0), "Apple"),
        (2, 2, unix(2024, 6, 3, 10, 0, 0), "Docs"),
    ]
    for vid, item, ut, title in visits:
        c.execute(
            "INSERT INTO history_visits (id, history_item, visit_time, title) VALUES (?,?,?,?)",
            (vid, item, cocoa(ut), title))
    db.commit()
    db.close()


def build_safari_cache(path):
    """Safari/WebKit `Cache.db`: cfurl_cache_response (time_stamp = TEXT UTC)."""
    db = fresh(path)
    c = db.cursor()
    c.execute(
        "CREATE TABLE cfurl_cache_response (entry_ID INTEGER PRIMARY KEY, version INTEGER, "
        "hash_value INTEGER, storage_policy INTEGER, request_key TEXT, time_stamp TEXT, "
        "partition TEXT)"
    )
    rows = [
        (1, "https://cdn.example.com/app.js", "2024-07-01 08:15:30"),
        (2, "https://img.example.net/logo.png", "2024-07-01 08:16:00"),
    ]
    for eid, url, ts in rows:
        c.execute(
            "INSERT INTO cfurl_cache_response (entry_ID, request_key, time_stamp) VALUES (?,?,?)",
            (eid, url, ts))
    db.commit()
    db.close()


def build_other(path):
    """A valid SQLite DB that is NOT a browser artifact (error path)."""
    db = fresh(path)
    db.execute("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)")
    db.execute("INSERT INTO notes (body) VALUES ('hello')")
    db.commit()
    db.close()


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build_chrome_history(os.path.join(here, "chrome_history.db"))
    build_firefox_places(os.path.join(here, "firefox_places.sqlite"))
    build_chrome_cookies(os.path.join(here, "chrome_cookies.db"))
    build_firefox_cookies(os.path.join(here, "firefox_cookies.sqlite"))
    build_safari_history(os.path.join(here, "safari_history.db"))
    build_safari_cache(os.path.join(here, "safari_cache.db"))
    build_other(os.path.join(here, "other.db"))
    print("WEBKIT_OFFSET_SECONDS =", WEBKIT_OFFSET_SECONDS)
    print("COCOA_OFFSET_SECONDS  =", COCOA_OFFSET_SECONDS)
    for label, y, mo, d, h, mi, s in [
        ("chrome visit LINK", 2024, 1, 15, 10, 30, 0),
        ("chrome visit TYPED", 2023, 12, 25, 8, 0, 0),
        ("chrome download", 2024, 1, 20, 9, 5, 0),
        ("firefox visit LINK", 2024, 3, 10, 9, 15, 0),
        ("firefox visit TYPED", 2022, 6, 1, 0, 0, 0),
        ("chrome cookie github", 2024, 5, 1, 8, 0, 0),
        ("chrome cookie example", 2023, 11, 20, 14, 30, 0),
        ("firefox cookie mozilla", 2024, 4, 10, 6, 0, 0),
        ("firefox cookie wiki", 2024, 1, 5, 12, 0, 0),
        ("safari visit apple", 2024, 6, 2, 16, 20, 0),
        ("safari visit docs", 2024, 6, 3, 10, 0, 0),
        ("safari cache appjs", 2024, 7, 1, 8, 15, 30),
        ("safari cache logo", 2024, 7, 1, 8, 16, 0),
    ]:
        u = unix(y, mo, d, h, mi, s)
        print(f"{label:24} unix={u:>12}  iso={iso(u)}")


if __name__ == "__main__":
    main()
