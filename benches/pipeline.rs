use skald_view::{Document, Limits};
use std::{hint::black_box, time::Instant};
fn main() {
    println!("fixture,scenario,median_ms,p95_ms,iterations,cached_bytes");
    for name in [
        "invoice-flate",
        "invoice-brotli",
        "report-brotli",
        "raster-jxl-0",
        "raster-jxl-1",
        "raster-jxl-2",
    ] {
        let source = std::fs::read(format!("examples/pdfs/{name}.pdf")).unwrap();
        let mut warmed = Document::open(source.clone(), Limits::default()).unwrap();
        black_box(warmed.render(0, 1.).unwrap());
        for scenario in ["open-first-page", "cached-first-page"] {
            let mut samples = Vec::new();
            for round in 0..55 {
                let start = Instant::now();
                if scenario == "open-first-page" {
                    let mut d = Document::open(source.clone(), Limits::default()).unwrap();
                    black_box(d.render(0, 1.).unwrap());
                } else {
                    black_box(warmed.render(0, 1.).unwrap());
                }
                if round >= 5 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{name},{scenario},{:.4},{:.4},50,{}",
                samples[25],
                samples[47],
                warmed.cached_bytes()
            );
        }
    }
}
