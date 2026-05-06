// Pending-uploads state machine for the chat composer.
//
// Single responsibility: track files the user has dropped/picked but not yet
// sent. Pure state + DOM rendering — no event wiring (that's gizza-app.js).

const MAX_FILE_BYTES = 10 * 1024 * 1024; // 10 MiB; matches video skill caps.

const pending = []; // [{ id, mime, filename, blob }]
let nextUploadId = 1;

/**
 * Add a File to the pending list. Returns `{ok: true, entry}` on success,
 * `{ok: false, error}` on validation failure (wrong mime / too large).
 *
 * The id is monotonically increasing and never reused, even after a remove —
 * so a chip identified by `upload_3` always means the third addPending call
 * since page load.
 */
export function addPending(file) {
    const isImage = typeof file.type === 'string' && file.type.startsWith('image/');
    const isVideo = typeof file.type === 'string' && file.type.startsWith('video/');
    if (!isImage && !isVideo) {
        return { ok: false, error: 'Only images and videos are supported.' };
    }
    if (typeof file.size === 'number' && file.size > MAX_FILE_BYTES) {
        return { ok: false, error: 'File too large; 10 MiB max.' };
    }
    const entry = {
        id: `upload_${nextUploadId++}`,
        mime: file.type,
        filename: file.name || (isImage ? 'image' : 'video'),
        blob: file,
    };
    pending.push(entry);
    return { ok: true, entry };
}

/** Remove the entry with the given id. No-op if not found. */
export function removePending(id) {
    const idx = pending.findIndex((e) => e.id === id);
    if (idx >= 0) pending.splice(idx, 1);
}

/** Clear all pending entries. Does NOT reset nextUploadId. */
export function clearPending() {
    pending.length = 0;
}

/** Snapshot of the current pending list. Returns a new array (safe to iterate). */
export function getPending() {
    return [...pending];
}

/**
 * Render the pending list into the given strip element.
 * Replaces existing children. Adds/removes the `empty` class on the strip.
 *
 * Each chip is `<span class="chip" data-id="upload_N">…<button class="remove">×</button></span>`.
 * The caller is responsible for wiring the click handler on `.remove` —
 * `removePending(id)` then `renderChips(strip)` again.
 */
export function renderChips(strip) {
    strip.replaceChildren();
    if (pending.length === 0) {
        strip.classList.add('empty');
        return;
    }
    strip.classList.remove('empty');
    for (const entry of pending) {
        const chip = document.createElement('span');
        chip.className = 'chip';
        chip.setAttribute('data-id', entry.id);

        const label = document.createElement('span');
        label.className = 'chip-label';
        label.textContent = entry.filename;
        chip.appendChild(label);

        const remove = document.createElement('button');
        remove.type = 'button';
        remove.className = 'remove';
        remove.setAttribute('aria-label', `Remove ${entry.filename}`);
        remove.textContent = '×';
        chip.appendChild(remove);

        strip.appendChild(chip);
    }
}

/** Test-only: reset all module state. Not exported in production paths. */
export function _resetForTests() {
    pending.length = 0;
    nextUploadId = 1;
}
