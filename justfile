default:
    @just --list

# Serve pkg/ over HTTP on the chosen port (default 8001).
serve port="8001":
    python3 -m http.server --directory pkg {{port}}

# Run the e2e smoke test.
test:
    cd tests && npx playwright test

# Render generic tool pages (no site config — this repo has none).
build-tools:
    cargo run --manifest-path tools/generator/Cargo.toml -- .
