// header.js — mega-menu open/close behavior + Tools search for the shared
// site chrome rendered by the gizza-chrome crate.
//
// DOM contract (ids come from chrome/src/lib.rs header()):
//   #explore-trigger  — <button> that opens/closes the mega-menu
//   #explore-panel    — the <div class="mega-menu"> panel
//   #explore-search   — <input type="search"> inside the mega-menu
//   #explore-results  — <ul> to fill with filtered tool rows (≤8)
//
// Fetch contract:
//   GET /tools/_index.json → [{slug, title, description, tags}, ...]
//   Fetched once (lazy, on first open); filtered client-side via filterTools.

import { filterTools } from './tools-index.js';

const INDEX_URL = '/tools/_index.json';
const MAX_RESULTS = 8;

function rowHtml(t) {
  const esc = (s) =>
    String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return (
    `<li><a class="mega-menu__result-link" href="/tools/${encodeURIComponent(t.slug)}/">` +
    `<span class="mega-menu__result-title">${esc(t.title)}</span>` +
    `<span class="mega-menu__result-desc">${esc(t.description)}</span>` +
    `</a></li>`
  );
}

function initHeader() {
  const trigger = document.getElementById('explore-trigger');
  const panel = document.getElementById('explore-panel');
  const searchInput = document.getElementById('explore-search');
  const resultsList = document.getElementById('explore-results');

  // Bail if the chrome is absent from this page (should not happen — both the
  // chat app and static tool pages render the shared gizza-chrome header).
  if (!trigger || !panel || !searchInput || !resultsList) return;

  let allTools = null; // cached after first fetch

  // ── Render filtered results ──────────────────────────────────────────────

  function render() {
    if (!allTools) return;
    const matches = filterTools(allTools, searchInput.value).slice(0, MAX_RESULTS);
    resultsList.innerHTML = matches.map(rowHtml).join('');
  }

  // ── Load index (once) ────────────────────────────────────────────────────

  async function loadOnce() {
    if (allTools) {
      render();
      return;
    }
    resultsList.innerHTML = '';
    try {
      const res = await fetch(INDEX_URL);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      allTools = await res.json();
      render();
    } catch (e) {
      console.error('[gizza] header tools index load failed:', e);
      // Leave resultsList empty; the user can re-open to retry.
    }
  }

  // ── Toggle open/close ────────────────────────────────────────────────────

  function open() {
    panel.removeAttribute('hidden');
    trigger.setAttribute('aria-expanded', 'true');
    searchInput.focus();
    loadOnce();
  }

  function close() {
    panel.setAttribute('hidden', '');
    trigger.setAttribute('aria-expanded', 'false');
  }

  function isOpen() {
    return !panel.hasAttribute('hidden');
  }

  trigger.addEventListener('click', () => {
    if (isOpen()) {
      close();
    } else {
      open();
    }
  });

  // Close on Escape key
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && isOpen()) {
      close();
      trigger.focus();
    }
  });

  // Close when clicking outside the mega-menu wrapper
  document.addEventListener('click', (e) => {
    if (!isOpen()) return;
    const wrapper = trigger.closest('.mega-menu-wrapper');
    if (wrapper && !wrapper.contains(e.target)) {
      close();
    }
  });

  // Live filter on input
  searchInput.addEventListener('input', render);
}

// Only wire DOM in a browser — keeps module importable under node:test.
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initHeader);
  } else {
    initHeader();
  }
}
