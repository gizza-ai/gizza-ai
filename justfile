default:
    @just --list

# Serve dist/ over HTTP on the chosen port (default 8001).
serve port="8001":
    python3 -m http.server --directory dist {{port}}

# Run the e2e smoke test.
test:
    cd tests && npx playwright test

# Generate sitemap.xml, robots.txt, and llms.txt into pkg/.
seo:
    GIZZA=cli/target/release/gizza scripts/gen-seo.sh
