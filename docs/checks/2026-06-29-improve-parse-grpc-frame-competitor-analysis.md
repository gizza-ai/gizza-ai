# gRPC Frame Parser competitor analysis (2026-06-29)

Tool: `parse-grpc-frame`

## Competitors reviewed

1. pbdecoder.online Protobuf Decoder
   - Browser tool for decoding protobuf payloads and debugging gRPC/microservice communication.
   - Strong protobuf wire-format decoding, but focuses on raw protobuf payloads rather than splitting gRPC's 5-byte message frames first.
2. Toolbox365 Protobuf Decoder
   - Online `protoc --decode_raw`-style decoder; documentation notes that users must strip the 5-byte gRPC frame header first.
   - Confirms the gap: users debugging captured gRPC DATA bytes need frame splitting before protobuf decode.
3. protobuf-decoder.netlify.app
   - Minimal no-schema protobuf decoder.
   - Useful for single payloads, but not a gRPC stream/frame parser.
4. Viadreams / Akousa / SEO Web Checker protobuf decoders
   - Generic protobuf wire-format decoders that turn binary data into readable field trees.
   - Typically expect a protobuf message, not concatenated gRPC frames with compression flags and big-endian lengths.
5. Wireshark gRPC dissector
   - Full packet/capture analysis with HTTP/2 and optional protobuf knowledge.
   - Powerful desktop workflow, but heavier than a quick paste-and-decode browser/CLI tool.

## In-model gaps and actions taken

- gRPC frame splitting: implemented parsing of one or more concatenated gRPC messages, each with 1-byte compression flag and 4-byte big-endian length.
- Protobuf payload decode: decodes uncompressed payloads without a `.proto` schema into field number, wire type, varint/fixed/length-delimited values, and recursively attempts nested messages.
- Input convenience: accepts hex or base64, with auto-detection; hex tolerates spaces, colons, and dashes.
- Compressed-frame handling: reports compressed frames with flag/length/payload hex and an explicit decompress-first note rather than pretending to decode compressed bytes.
- Output modes: supports compact text and structured JSON for scripting.
- Multi-surface coverage: chat/CLI/page all share the same pure Rust core and schema drift guard.
- Page copy: explains gRPC's 5-byte message prefix, multi-frame streams, schema-less protobuf limitations, and privacy/local execution.

## Out-of-model or intentionally not implemented

- Decompression: gRPC compressed messages require knowing the negotiated codec (gzip, zstd, etc.); this tool reports the frame and asks users to decompress before payload decode.
- `.proto`-aware names/types: without descriptors, fields are necessarily shown by number and wire type.
- HTTP/2 capture parsing: the tool expects the raw gRPC message stream (DATA payload), not a full pcap/HAR/HTTP2 frame log.
- gRPC-Web base64/text framing: future extension; current scope is native gRPC length-prefixed message framing.

## Verification snapshot

- `cargo test --workspace` from `blocks/parse-grpc-frame`: passed.
- `wafer build` from `blocks/parse-grpc-frame`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/parse-grpc-frame/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/parse-grpc-frame/`.
- `gizza tool parse-grpc-frame input='00 00 00 00 03 08 96 01' encoding=hex format=text`: passed.
- `cd tests && xvfb-run npx playwright test tool-page-parse-grpc-frame.spec.ts --timeout=120000 --reporter=line`: passed.
