fn main() {
    let listing = "12582912\t./media/intro.mp4\n8388608\t./media/promo.mp4\n2097152\t./img/hero.png\n1048576\t./img/logo.png\n524288\t./img/team.jpg\n204800\t./docs/spec.pdf\n40960\t./src/app.js\n20480\t./src/util.js\n8192\t./README.md\n1024\t./Makefile\n";
    let o = gizza_ai_disk_usage_by_filetype_core::Options::default();
    println!("{}", gizza_ai_disk_usage_by_filetype_core::run(listing, &o).unwrap());
    println!("---- table/category ----");
    let o2 = gizza_ai_disk_usage_by_filetype_core::Options { group_by: "category".into(), format: "table".into(), ..Default::default() };
    println!("{}", gizza_ai_disk_usage_by_filetype_core::run(listing, &o2).unwrap());
}
