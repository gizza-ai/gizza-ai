# WebSocket Frame Parser competitor analysis (2026-06-29)

Tool: `parse-websocket-frame`

## Competitors reviewed

1. DevOven / HttpStatus.com online WS frame decoders
   - Browser tools that take a pasted raw frame (hex) and break out FIN, RSV1–3, opcode,
     MASK flag, masking key, payload length, and the unmasked payload, validating against
     RFC 6455.
   - Closest direct competitors. Main gaps: standalone web pages only (not scriptable/CLI),
     and multi-frame handling quality varies.
2. Wireshark WebSocket dissector
   - Captures live traffic and dissects each WS frame: FIN, reserved bits, opcode, mask flag,
     payload length, masking key, masked + auto-unmasked payload.
   - Does NOT accept pasted raw bytes — needs a live/pcap capture, so it can't decode an
     isolated frame snippet pulled from a log or a doc.
3. Chrome DevTools Messages tab (Network panel)
   - Lists sent/received WS messages with decoded payload, length, and timing.
   - Works only on a live connection in your own browser tab and shows already-decoded
     application messages — it never exposes the raw frame bytes, the FIN/RSV/MASK bits, or
     the masking key, and can't unmask arbitrary bytes.
4. npm `ws` (Receiver) / `ws-low-level`
   - Node.js libraries that parse an incoming byte stream into opcode/FIN/mask/payload and
     unmask it.
   - Programmatic dependencies, not paste-and-go: you must write code, feed a Buffer, and they
     target live streaming sockets, not one-off inspection of a hex string.
5. Generic hex / CyberChef-style protocol decoders
   - Flexible byte tools that can XOR-unmask and slice bytes.
   - No built-in WS frame schema — the user must know the offsets and bit layout by hand; no
     automatic opcode naming or RFC validation.

## In-model gaps and actions taken (already implemented)

- Isolated-frame decode: accepts a single pasted frame as hex or base64 (auto-detected) with
  no live connection or pcap capture required.
- Low-level field surfacing: reports FIN, RSV1/RSV2/RSV3, opcode (number + human name), MASK
  flag, masking key, and payload length — the fields browser inspectors hide.
- Auto-unmask: XOR-unmasks the payload with `key[i % 4]` and shows it as hex plus, for text
  frames, UTF-8 — no code to write.
- Extended length: handles the 126 (`uint16`) and 127 (`uint64`) big-endian length markers.
- Control-frame detail: names continuation/text/binary/close/ping/pong and splits a close
  frame into its `{code, reason}`.
- Robustness: truncated/too-short frames return a clear, specific error instead of garbage;
  trailing bytes after the first frame are reported, not silently dropped.
- Output modes: compact `text` summary or structured `json` for scripting.
- Multi-surface coverage: chat/CLI/page share one pure-Rust core and a schema drift guard, so
  the tool is scriptable and offline — a gap every competitor above leaves open.
- Page copy: explains the two-byte header layout, the masking rationale, the 126/127 length
  markers, and the local/private execution model, with FAQ accordions.

## Out-of-model or intentionally not implemented

- Live capture / pcap ingestion: this tool decodes pasted bytes, not a packet capture or an
  HTTP/2-over-TLS session (Wireshark/DevTools territory, requires a backend/desktop capture).
- `permessage-deflate` decompression: an RSV1-compressed payload needs the negotiated
  extension state; the tool flags the RSV bit but does not decompress.
- Multi-frame reassembly: only the first frame is decoded (trailing bytes are noted); stitching
  fragmented messages across frames is out of scope for a single-frame inspector.
- The opening HTTP `Upgrade: websocket` handshake is plain HTTP, not a frame — out of scope.

## Verification snapshot

- `CARGO_BUILD_JOBS=1 cargo test --workspace` from `blocks/parse-websocket-frame`: passed
  (20 unit + drift-guard tests).
- `wafer build` from `blocks/parse-websocket-frame`: passed; produced `target/block.wasm`
  (320 KiB, validated).
- `wasm-pack build blocks/parse-websocket-frame/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered the page +
  `pkg/tools/parse-websocket-frame/`.
- `gizza tool parse-websocket-frame input='81 05 48 65 6c 6c 6f' encoding=hex format=text`: passed
  (decoded a text frame to "Hello"); masked-frame json surface also verified.
- `cd tests && xvfb-run npx playwright test tool-page-parse-websocket-frame.spec.ts`: 5 passed
  (default json, masked unmask, close-frame text, base64 input, query-param deep-link).
