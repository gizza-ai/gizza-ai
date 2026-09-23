fn main() {
    use gizza_ai_pagerank_ranker_core::{run, Options};
    let g = "home -> about\nhome -> pricing\nabout -> pricing\npricing -> home\nblog -> pricing";
    println!("{}", run(g, &Options::default()).unwrap());
    println!("--- json/top2 ---");
    let mut o = Options::default();
    o.format = "json".into();
    o.top = 2;
    println!("{}", run(g, &o).unwrap());
    println!("--- matrix ---");
    println!(
        "{}",
        run("0 1 1\n0 0 1\n1 0 0", &Options::default()).unwrap()
    );
    println!("--- csv weighted undirected ---");
    let mut o = Options::default();
    o.format = "csv".into();
    o.weighted = true;
    o.directed = false;
    println!("{}", run("a - b : 3\nb - c : 1", &o).unwrap());
}
