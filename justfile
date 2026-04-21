default:
    @just --list

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
