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

# Provision sql.js static assets from solobase-web (builds its Makefile recipe
# on first use). bridge.js imports /sql-wasm-esm.js and fetches /sql-wasm.wasm
# at runtime for the BrowserDatabaseService.
sql-assets:
    #!/usr/bin/env bash
    set -euo pipefail
    SB_PKG=../solobase/crates/solobase-web/pkg
    if [ ! -f "$SB_PKG/sql-wasm-esm.js" ] || [ ! -f "$SB_PKG/sql-wasm.wasm" ]; then
        echo "Building solobase-web's sql.js assets first..."
        (cd ../solobase/crates/solobase-web && make pkg/sql-wasm-esm.js)
    fi

# Assemble dist/ from site/ + pkg/.
#
# Stamps a BUILD_ID (git SHA if available, else timestamp) into dist/sw.js.
# That changes the SW script's bytes on every build, so Chrome detects a
# new SW and reinstalls it on the next page visit — users see updates
# without having to unregister the old worker. The same BUILD_ID also
# cache-busts the dynamic `import('./gizza_ai.js?v=...')` inside sw.js so
# the new SW actually loads fresh wasm instead of the cached copy.
build: build-wasm sql-assets
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf dist
    mkdir -p dist
    cp site/* dist/
    cp pkg/gizza_ai.js dist/
    cp pkg/gizza_ai_bg.wasm dist/
    # wasm-pack emits `snippets/` containing JS modules referenced from the
    # wasm-bindgen output (our site/bridge.js). Without it, gizza_ai.js's
    # `import … from './snippets/.../bridge.js'` 404s and the SW fails to start.
    cp -r pkg/snippets dist/
    cp ../solobase/crates/solobase-web/pkg/sql-wasm-esm.js dist/
    cp ../solobase/crates/solobase-web/pkg/sql-wasm.wasm dist/

    # Stamp BUILD_ID into sw.js (cache-bust the wasm-bindgen import).
    BUILD_ID=$(git rev-parse --short HEAD 2>/dev/null || date +%s)
    sed -i "s|__BUILD_ID__|${BUILD_ID}|g" dist/sw.js
    echo "build: BUILD_ID=${BUILD_ID} stamped into dist/sw.js"

# Serve dist/ on localhost:8000.
serve: build
    python3 -m http.server --directory dist 8000

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
