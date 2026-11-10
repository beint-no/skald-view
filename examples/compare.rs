use std::{io::BufReader, path::PathBuf};
fn png(path: PathBuf) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut data = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut data).unwrap();
    data.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Rgba => data,
        _ => panic!("reference must be RGB/RGBA"),
    };
    (info.width, info.height, rgba)
}
fn main() {
    let mut args = std::env::args().skip(1);
    let actual = PathBuf::from(args.next().unwrap());
    let reference = PathBuf::from(args.next().unwrap());
    let mut files = std::fs::read_dir(&reference)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect::<Vec<_>>();
    files.sort();
    let mut rows = Vec::new();
    for path in files {
        let name = path.file_name().unwrap();
        let (w, h, a) = png(actual.join(name));
        let (rw, rh, b) = png(path.clone());
        assert_eq!((w, h), (rw, rh), "{} dimensions", path.display());
        let differences = a
            .iter()
            .zip(&b)
            .map(|(x, y)| x.abs_diff(*y))
            .collect::<Vec<_>>();
        let mae = differences.iter().map(|&v| v as f64).sum::<f64>() / differences.len() as f64;
        let high =
            differences.iter().filter(|&&v| v > 32).count() as f64 / differences.len() as f64;
        rows.push(serde_json::json!({"page":name.to_string_lossy(),"width":w,"height":h,"mae":mae,"fractionAbove32":high}));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
