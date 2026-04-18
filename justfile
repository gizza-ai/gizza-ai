default:
    @just --list

# Build WASM skill blocks.
build-skills:
    #!/usr/bin/env bash
    set -euo pipefail
    for dir in blocks/*/; do
        if [ -f "$dir/Cargo.toml" ]; then
            echo "Building $dir"
            (cd "$dir" && /home/joris/Programs/suppers-ai/workspace/wafer-run/target/release/wafer build)
        fi
    done

# Build the main gizza-ai wasm.
build-wasm: build-skills
    wasm-pack build --target web --out-dir pkg

# Assemble dist/ from site/ + pkg/.
build: build-wasm
    rm -rf dist
    mkdir -p dist
    cp site/* dist/
    cp pkg/gizza_ai.js dist/
    cp pkg/gizza_ai_bg.wasm dist/
    # wasm-pack also generates gizza_ai.d.ts, package.json, README.md — ignore them.

# Serve dist/ on localhost:8000.
serve: build
    python3 -m http.server --directory dist 8000

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
