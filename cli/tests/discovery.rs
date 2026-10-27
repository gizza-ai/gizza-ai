use gizza_cli::runtime;

#[tokio::test]
async fn lists_calculator_with_schema() {
    let rt = runtime::boot().await.expect("boot");
    let tools = rt.tools();
    let calc = tools
        .iter()
        .find(|t| t.name == "gizza-ai/calculator")
        .expect("calc present");
    assert!(
        calc.description.to_lowercase().contains("expression"),
        "description should mention 'expression', got: {}",
        calc.description
    );
    assert_eq!(calc.parameters["required"][0], "expr");
}

#[tokio::test]
async fn video_tools_present_despite_stale_manifest() {
    let rt = runtime::boot().await.expect("boot");
    assert!(
        rt.tools().iter().any(|t| t.name == "gizza-ai/video-trim"),
        "video-trim must come from info(), not manifest"
    );
}

/// Every block crate in the repo ships a committed artifact, and the CLI lists
/// each one as a tool. `boot()` already fails on an artifact that will not
/// load; this catches the quieter losses: a block whose `target/block.wasm`
/// is missing (build.rs skips it), or one that loads without a `SkillTool`.
#[tokio::test]
async fn lists_every_block_in_the_repo() {
    let blocks_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../blocks");
    let mut slugs: Vec<String> = std::fs::read_dir(&blocks_dir)
        .expect("read blocks/")
        .map(|entry| entry.expect("blocks/ entry").path())
        .filter(|dir| dir.join("Cargo.toml").is_file())
        .map(|dir| dir.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    slugs.sort();
    assert!(
        !slugs.is_empty(),
        "no block crates under {}",
        blocks_dir.display()
    );

    let rt = runtime::boot().await.expect("boot");
    let listed: Vec<&str> = rt.tools().iter().map(|t| t.name.as_str()).collect();
    let missing: Vec<&String> = slugs
        .iter()
        .filter(|slug| rt.tool(slug).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "{} block(s) not listed as tools: {:?}",
        missing.len(),
        missing
    );
    assert_eq!(
        listed.len(),
        slugs.len(),
        "the CLI lists a tool that has no block crate"
    );
}
