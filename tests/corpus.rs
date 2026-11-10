use skald_view::{Document, Limits};
#[test]
fn entire_pinned_skald_application_corpus_renders() {
    let root = std::path::Path::new("tests/skald-corpus");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let mut pages = 0;
    for file in manifest["files"].as_array().unwrap() {
        let name = file["file"].as_str().unwrap();
        let mut d = Document::open(std::fs::read(root.join(name)).unwrap(), Limits::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        for page in 0..d.page_count() {
            let frame = d
                .render(page, 1.)
                .unwrap_or_else(|e| panic!("{name} page {page}: {e}"));
            assert_eq!(
                frame.rgba.len(),
                frame.width as usize * frame.height as usize * 4
            );
            pages += 1;
        }
    }
    assert_eq!(pages, 102);
}
#[test]
fn corpus_exposes_safe_links_and_readonly_signature_metadata() {
    let root = "tests/skald-corpus/";
    let d = Document::open(
        std::fs::read(format!("{root}modern-chrome.pdf")).unwrap(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        d.page_info(0).unwrap().links[0].uri,
        "https://github.com/beint-no/skald-pdf"
    );
    let d = Document::open(
        std::fs::read(format!("{root}reai-16-faktura-signert.pdf")).unwrap(),
        Limits::default(),
    )
    .unwrap();
    assert!(d.page_info(0).unwrap().signature_fields_present);
}
#[test]
fn frame_cache_is_immutable_bounded_and_keyed_by_page_and_scale() {
    let source = std::fs::read("examples/pdfs/report-brotli.pdf").unwrap();
    let mut d = Document::open(source.clone(), Limits::default()).unwrap();
    let a = d.render(0, 1.).unwrap();
    let b = d.render(0, 1.).unwrap();
    assert!(std::sync::Arc::ptr_eq(&a.rgba, &b.rgba));
    let c = d.render(1, 1.).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&a.rgba, &c.rgba));
    assert_eq!(a.rgba, d.render(0, 1.).unwrap().rgba);
    d.clear_frame_cache();
    assert_eq!(d.frame_cache_bytes(), 0);
    let mut uncached = Document::open(
        source,
        Limits {
            frame_cache_bytes: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let no_cache = uncached.render(0, 1.).unwrap();
    assert_eq!(a.rgba, no_cache.rgba);
    assert_eq!(uncached.frame_cache_bytes(), 0);
}
