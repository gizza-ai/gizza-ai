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

# Assemble dist/ from site/ + pkg/ + solobase-browser framework assets.
#
# solobase-browser's `export-assets` bin vendors sql.js, writes the
# parameterized sw.js/loader.js/index.html templates, content-hashes
# the wasm-pack output, and renders the templates with the given
# --app-name / --app-title / --boot-redirect placeholders. We then
# overwrite the default index.html with gizza's branded one and add
# gizza's UI scripts.
build: build-wasm
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf dist
    mkdir -p dist
    cp pkg/gizza_ai.js pkg/gizza_ai_bg.wasm dist/
    # wasm-pack emits `snippets/` referenced from the wasm-bindgen output.
    cp -r pkg/snippets dist/
    cargo run --manifest-path ../solobase/Cargo.toml -p solobase-browser --release --bin export-assets -- dist/ \
        --repo-dir "$(pwd)" \
        --app-name gizza-ai \
        --app-title "Gizza AI" \
        --boot-redirect / \
        --extra-bypass-prefix "/ai-bridge.js,/gizza-app.js,/gizza.css"
    # Gizza-branded index.html + app JS overwrite the framework defaults.
    cp site/index.html site/gizza-app.js site/gizza.css site/ai-bridge.js dist/

# Serve dist/ on localhost:8000.
serve: build
    python3 -m http.server --directory dist 8000

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
