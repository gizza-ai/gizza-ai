# List tasks.
default:
    @just --list

# Build every WASM skill block under blocks/, then the main wasm.
build:
    @echo "Build logic wired up in Plan B Task 7."

# Serve dist/ on localhost:8000.
serve: build
    python3 -m http.server --directory dist 8000

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
