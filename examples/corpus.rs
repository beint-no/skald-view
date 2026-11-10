use skald_view::{Document, Limits};
use std::{path::PathBuf, time::Instant};
fn main() {
    let directory = std::env::args().nth(1).expect("corpus directory");
    let mut paths = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut records = Vec::new();
    for path in paths {
        let source = std::fs::read(&path).unwrap();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let start = Instant::now();
        let result = (|| {
            let mut d = Document::open(source.clone(), Limits::default())?;
            let mut pages = Vec::new();
            for index in 0..d.page_count() {
                let f = d.render(index, 1.)?;
                if let Some(output) = std::env::args().nth(2) {
                    let output = PathBuf::from(output);
                    std::fs::create_dir_all(&output).unwrap();
                    let file =
                        std::fs::File::create(output.join(format!("{name}-{}.png", index + 1)))
                            .unwrap();
                    let mut encoder = png::Encoder::new(file, f.width, f.height);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(&f.rgba)
                        .unwrap();
                }
                pages.push(serde_json::json!({"page":index+1,"width":f.width,"height":f.height,"textRuns":f.text.len()}));
            }
            Ok::<_, skald_view::Error>((pages, d.cached_bytes()))
        })();
        records.push(match result{Ok((pages,cache))=>serde_json::json!({"name":name,"sourceBytes":source.len(),"pages":pages,"cachedBytes":cache,"elapsedMs":start.elapsed().as_secs_f64()*1000.}),Err(e)=>serde_json::json!({"name":name,"error":e.to_string()})});
    }
    println!("{}", serde_json::to_string_pretty(&records).unwrap());
}
