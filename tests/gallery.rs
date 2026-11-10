use skald_view::{Document, Limits};
use std::path::Path;
fn load(name: &str) -> Document {
    Document::open(
        std::fs::read(format!("examples/pdfs/{name}.pdf")).unwrap(),
        Limits::default(),
    )
    .unwrap()
}
fn frame(name: &str) -> skald_view::RenderedPage {
    load(name).render(0, 1.).unwrap()
}
#[test]
fn gallery_renders_every_supported_example() {
    for entry in std::fs::read_dir("examples/pdfs").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("pdf") {
            continue;
        }
        let source = std::fs::read(&path).unwrap();
        if path.file_name().unwrap() == "rejected-legacy.pdf" {
            assert!(Document::open(source, Limits::default()).is_err());
            continue;
        }
        let mut doc = Document::open(source, Limits::default())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for page in 0..doc.page_count() {
            let result = doc
                .render(page, 1.)
                .unwrap_or_else(|e| panic!("{} page {page}: {e}", path.display()));
            assert_eq!(
                result.rgba.len(),
                result.width as usize * result.height as usize * 4
            );
            assert!(result.rgba.as_chunks::<4>().0.iter().any(|p| p[0] < 240));
        }
    }
}
#[test]
fn brotli_preserves_exact_invoice_pixels_and_text() {
    let a = frame("invoice-flate");
    let b = frame("invoice-brotli");
    assert_eq!(a.rgba, b.rgba);
    assert_eq!(
        serde_json::to_string(&a.text).unwrap(),
        serde_json::to_string(&b.text).unwrap()
    );
    assert!(
        a.text
            .iter()
            .map(|t| t.text.as_str())
            .collect::<String>()
            .contains("13 000")
    );
}
#[test]
fn lossless_jxl_preserves_raster_pixels() {
    assert_eq!(frame("raster-flate").rgba, frame("raster-jxl-0").rgba);
}
#[test]
fn lossy_jxl_has_real_bounded_pixel_differences() {
    let control = frame("raster-flate");
    for name in ["raster-jxl-1", "raster-jxl-2"] {
        let candidate = frame(name);
        assert_ne!(control.rgba, candidate.rgba);
        let mae = control
            .rgba
            .iter()
            .zip(candidate.rgba.iter())
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum::<f64>()
            / control.rgba.len() as f64;
        assert!(mae < 4., "{name}: MAE {mae}");
    }
}
#[test]
fn brotli_mask_matches_flate_mask() {
    assert_eq!(frame("mask-flate").rgba, frame("mask-brotli").rgba);
}
#[test]
fn unicode_survives_glyph_rendering() {
    let f = frame("unicode-brotli");
    let text = f
        .text
        .iter()
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(text.contains("æøå ÆØÅ"));
    assert!(text.contains("Καλημέρα"));
    assert!(text.contains("Привет мир"));
}
#[test]
fn cached_render_does_not_grow_decoded_cache() {
    let mut d = load("invoice-brotli");
    let width = d.page_info(0).unwrap().width;
    d.render(0, 1.).unwrap();
    let bytes = d.cached_bytes() - d.frame_cache_bytes();
    for scale in [0.5, 1., 1.5] {
        let f = d.render(0, scale).unwrap();
        assert_eq!(f.width, (width * scale).ceil() as u32);
        assert_eq!(bytes, d.cached_bytes() - d.frame_cache_bytes());
    }
}
#[test]
fn rejects_invalid_render_scales_and_pixel_budgets() {
    let mut d = load("invoice-flate");
    for s in [0., -1., f64::NAN, f64::INFINITY, 9.] {
        assert!(d.render(0, s).is_err());
    }
    assert!(d.render(999, 1.).is_err());
    let limits = Limits {
        render_pixels: 100,
        ..Default::default()
    };
    let mut d = Document::open(
        std::fs::read("examples/pdfs/invoice-flate.pdf").unwrap(),
        limits,
    )
    .unwrap();
    assert!(d.render(0, 1.).is_err());
}
#[test]
fn source_and_cache_limits_fail_explicitly() {
    let source = std::fs::read("examples/pdfs/invoice-flate.pdf").unwrap();
    assert!(
        Document::open(
            source.clone(),
            Limits {
                source_bytes: 32,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        Document::open(
            source,
            Limits {
                cached_bytes: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
}
fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    bytes.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgb => bytes
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Rgba => bytes,
        _ => panic!("unexpected PNG type"),
    };
    (info.width, info.height, rgba)
}
#[test]
fn independently_rendered_mupdf_goldens() {
    for (fixture, golden) in [
        ("invoice-flate", "invoice"),
        ("vectors-brotli", "vectors"),
        ("mask-flate", "mask"),
        ("unicode-brotli", "unicode"),
        ("report-brotli", "report"),
        ("text-paint-modes", "text-paint-modes"),
    ] {
        let f = frame(fixture);
        let (w, h, rgba) = read_png(Path::new(&format!("tests/references/{golden}.png")));
        assert_eq!((f.width, f.height), (w, h));
        let mae = f
            .rgba
            .iter()
            .zip(&rgba)
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum::<f64>()
            / rgba.len() as f64;
        println!("{fixture}: independent pixel MAE {mae}");
        assert!(mae < 2.5, "{fixture} MAE {mae}");
    }
}

#[test]
fn evicts_old_scenes_for_long_documents_instead_of_growing_forever() {
    let limits = Limits {
        cached_bytes: 4 << 20,
        ..Default::default()
    };
    let mut d = Document::open(
        std::fs::read("examples/pdfs/report-brotli.pdf").unwrap(),
        limits,
    )
    .unwrap();
    let first = d.render(0, 1.).unwrap().rgba;
    for page in 1..d.page_count() {
        d.render(page, 1.).unwrap();
        assert!(d.cached_bytes() <= 4 << 20);
    }
    assert_eq!(first, d.render(0, 1.).unwrap().rgba);
}

#[test]
fn gray_jxl_and_jxl_soft_masks_match_lossless_controls() {
    assert_eq!(frame("gray-flate").rgba, frame("gray-jxl-0").rgba);
    assert_eq!(frame("mask-flate").rgba, frame("mask-jxl").rgba);
    assert_ne!(frame("gray-flate").rgba, frame("gray-jxl-2").rgba);
}
#[test]
fn shared_images_do_not_decode_again_per_page() {
    let mut d = load("shared-image-report");
    let first = d.render(0, 1.).unwrap().rgba;
    let before = d.cached_bytes();
    for page in 1..d.page_count() {
        assert_eq!(first, d.render(page, 1.).unwrap().rgba);
    }
    assert!(d.cached_bytes() - before < 100_000);
}
