// mascot.js — shared animated gizza mascot (still + video + eyes).
//
// Single source of truth for the living crab mascot's behavior, used by BOTH
// the chat (site/gizza-app.js) and the static apex chooser (site/index.html).
// The mascot is a 3-layer composite: a still PNG (eyes-less), a video that
// overlays it during animation, and pupil sprites that track the user's cursor
// while a still is showing.
//
// Modes / poses:
//   resting       — gis_no_eyes.png + pupils (boot state)
//   video-idle    — gis_video_idle.mp4 (the "gis a job" reveal), no pupils
//   sign          — gis_a_job_no_eyes.png + pupils (after idle animation)
//   typing        — gis_video_typing_loop.mp4 looping, no pupils (chat only)
//   typing-finish — gis_video_typing_finish.mp4 (tail) once, no pupils (chat only)
//
// Flow: resting → 10s no activity → video-idle → ended → sign (stays).
// In CHAT mode, on composer submit: → typing; on finally: → typing-finish →
// ended → sign. In LANDING mode the typing states are never entered — the
// chooser only ever shows resting / idle-video / sign + the eyes following the
// cursor.
//
// The asset paths are SW-bypassed static files (see solobase.toml
// extra_bypass_prefix): /gis_no_eyes.png, /gis_a_job_no_eyes.png, /eye.png,
// /gis_video_idle.mp4, /gis_video_typing_loop.mp4, /gis_video_typing_finish.mp4.

const RESTING_SRC = '/gis_no_eyes.png';
const SIGN_SRC = '/gis_a_job_no_eyes.png';
const IDLE_VIDEO = '/gis_video_idle.mp4';
const TYPING_LOOP_SRC = '/gis_video_typing_loop.mp4';
const TYPING_FINISH_SRC = '/gis_video_typing_finish.mp4';
const IDLE_DELAY_MS = 10_000;

/**
 * Wire up the animated mascot inside `rootEl` (the `.brand-mascot` element).
 *
 * @param {Element} rootEl  the `.brand-mascot` container (still + video + eyes).
 * @param {object}  [opts]
 * @param {boolean} [opts.landing=false]  landing/idle mode — eyes follow + idle
 *        video plays, but the chat-typing states are never entered. The chat
 *        passes `false` (the default) so its typing state machine stays live.
 * @returns {{ startTyping: function, stopTyping: function, destroy: function } | null}
 *        a controller, or null if `rootEl` is missing its sub-elements. The
 *        chat drives `startTyping`/`stopTyping`; both are no-ops in landing mode.
 */
export function initMascot(rootEl, opts = {}) {
    if (!rootEl) return null;
    const landing = opts.landing === true;

    const brandStill = rootEl.querySelector('.brand-still');
    const brandVideo = rootEl.querySelector('.brand-video');
    const eyes = rootEl.querySelectorAll('.brand-eye');
    if (!brandStill || !brandVideo) return null;

    let brandMode = 'resting';
    let idleTimer = null;

    const absUrl = (rel) => new URL(rel, location.href).href;
    const videoSrcIs = (rel) => brandVideo.currentSrc === absUrl(rel);

    function clearIdleTimer() {
        if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
    }

    function showStill(src, pose) {
        brandStill.src = src;
        brandStill.hidden = false;
        brandVideo.hidden = true;
        try { brandVideo.pause(); } catch (_) {}
        rootEl.dataset.pose = pose;
    }

    function showVideo(src, { loop }) {
        brandStill.hidden = true;
        brandVideo.hidden = false;
        brandVideo.loop = loop;
        rootEl.dataset.pose = 'video';
        const start = () => {
            try { brandVideo.currentTime = 0; } catch (_) {}
            brandVideo.play().catch(() => {});
        };
        if (!videoSrcIs(src)) {
            brandVideo.src = src;
            brandVideo.addEventListener('loadedmetadata', start, { once: true });
        } else {
            start();
        }
    }

    function enterResting() {
        brandMode = 'resting';
        showStill(RESTING_SRC, 'resting');
        // After 10s of no activity, play the "gis a job" reveal video.
        clearIdleTimer();
        idleTimer = setTimeout(() => enterVideoIdle(), IDLE_DELAY_MS);
    }

    function enterVideoIdle() {
        if (brandMode === 'typing' || brandMode === 'typing-finish') return;
        brandMode = 'video-idle';
        clearIdleTimer();
        showVideo(IDLE_VIDEO, { loop: false });
    }

    function enterSign() {
        brandMode = 'sign';
        clearIdleTimer();
        showStill(SIGN_SRC, 'sign');
    }

    function startTyping() {
        // No typing states in landing mode — the chooser never animates typing.
        if (landing) return;
        clearIdleTimer();
        brandMode = 'typing';
        showVideo(TYPING_LOOP_SRC, { loop: true });
    }

    function stopTyping() {
        if (landing) return;
        if (brandMode !== 'typing') return;
        brandMode = 'typing-finish';
        showVideo(TYPING_FINISH_SRC, { loop: false });
    }

    function onEnded() {
        if (brandMode === 'video-idle' || brandMode === 'typing-finish') {
            enterSign();
        }
    }
    brandVideo.addEventListener('ended', onEnded);

    // ─── Pupil mouse-tracking ──────────────────────────────────────────────
    // Each eye socket has overflow:hidden — the pupil image translates
    // within its socket based on cursor angle/distance, mirroring the
    // solobase-site Hero approach.
    function onMouseMove(e) {
        if (rootEl.dataset.pose === 'video') return;
        requestAnimationFrame(() => {
            for (const eye of eyes) {
                const socket = eye.parentElement;
                const sr = socket.getBoundingClientRect();
                const er = eye.getBoundingClientRect();
                const cx = sr.left + sr.width / 2;
                const cy = sr.top + sr.height / 2;
                const dx = e.clientX - cx;
                const dy = e.clientY - cy;
                const angle = Math.atan2(dy, dx);
                const distance = Math.hypot(dx, dy);
                const maxX = (sr.width - er.width) / 2;
                const maxY = (sr.height - er.height) / 2;
                const scale = Math.min(distance / 200, 1);
                const mx = Math.cos(angle) * maxX * scale;
                const my = Math.sin(angle) * maxY * scale;
                eye.style.transform = `translate(${mx}px, ${my}px)`;
            }
        });
    }
    function onMouseLeave() {
        for (const eye of eyes) eye.style.transform = 'translate(0, 0)';
    }
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseleave', onMouseLeave);

    enterResting();

    return {
        startTyping,
        stopTyping,
        destroy() {
            clearIdleTimer();
            brandVideo.removeEventListener('ended', onEnded);
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseleave', onMouseLeave);
        },
    };
}
