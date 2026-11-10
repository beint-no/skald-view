use skald_view::{Document, Limits};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("input.pdf");
    let output = args.next().expect("output.png");
    let index = args
        .next()
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let mut doc = Document::open(std::fs::read(path)?, Limits::default())?;
    let start = std::time::Instant::now();
    let frame = doc.render(index, 1.)?;
    let file = std::fs::File::create(output)?;
    let mut png = png::Encoder::new(file, frame.width, frame.height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&frame.rgba)?;
    println!(
        "page {index}, text runs {}, tracked cache bytes {}, elapsed {:?}",
        frame.text.len(),
        doc.cached_bytes(),
        start.elapsed()
    );
    Ok(())
}
