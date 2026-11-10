use skald_view::{Document, Limits};
use std::{hint::black_box, time::Instant};
fn main() {
    let source = std::fs::read(std::env::args().nth(1).expect("PDF")).unwrap();
    let mut d = Document::open(source, Limits::default()).unwrap();
    let first = d.render(0, 1.).unwrap();
    let first_cache = d.cached_bytes();
    for p in 1..d.page_count() {
        black_box(d.render(p, 1.).unwrap());
    }
    let all_cache = d.cached_bytes();
    let start = Instant::now();
    for _ in 0..100 {
        black_box(d.render(0, 1.).unwrap());
    }
    println!(
        "{}",
        serde_json::json!({"pages":d.page_count(),"firstCacheBytes":first_cache,"allPagesCacheBytes":all_cache,"finalCacheBytes":d.cached_bytes(),"warmMeanMs":start.elapsed().as_secs_f64()*10.,"frameBytes":first.rgba.len()})
    );
}
