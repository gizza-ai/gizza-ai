// Render a skill's `_for_ui` payload into a tool-call row.
// Returns the appended element, or null if nothing was rendered.
//
// Recognised mimes:
//   image/* → <img src=... alt=filename>
//   video/* → <video src=... controls aria-label=filename>
//   anything else → nothing rendered.
//
// Validates that data_url starts with "data:" — defends against a buggy
// skill or future wire-format drift.
export function renderToolAttachment(row, forUi) {
    if (!forUi || typeof forUi !== 'object') return null;
    const dataUrl = forUi.data_url;
    if (typeof dataUrl !== 'string' || !dataUrl.startsWith('data:')) return null;
    const mime = typeof forUi.mime === 'string' ? forUi.mime : '';
    const filename = typeof forUi.filename === 'string' ? forUi.filename : '';

    let el;
    if (mime.startsWith('image/')) {
        el = document.createElement('img');
        el.src = dataUrl;
        el.alt = filename;
    } else if (mime.startsWith('video/')) {
        el = document.createElement('video');
        el.src = dataUrl;
        el.setAttribute('controls', '');
        if (filename) el.setAttribute('aria-label', filename);
    } else {
        return null;
    }

    el.className = 'tool-attachment';
    row.appendChild(el);
    return el;
}
