use skald_view::{Document, Limits};
use std::io::Write;
fn pdf(content: &[u8], page_extra: &str, tree_extra: &str) -> Vec<u8> {
    let mut source = b"%PDF-2.0\n".to_vec();
    let objects=[b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),format!("<< /Type /Pages /Count 1 /Kids [3 0 R] {tree_extra} >>").into_bytes(),format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> /Contents 4 0 R {page_extra} >>").into_bytes(),{let mut v=format!("<< /Length {} >>\nstream\n",content.len()).into_bytes();v.extend(content);v.extend(b"\nendstream");v}];
    let mut offsets = vec![0];
    for (i, value) in objects.iter().enumerate() {
        offsets.push(source.len());
        writeln!(source, "{} 0 obj", i + 1).unwrap();
        source.extend(value);
        source.extend(b"\nendobj\n");
    }
    let xref = source.len();
    offsets.push(xref);
    let mut entries = Vec::new();
    for (i, offset) in offsets.iter().enumerate() {
        entries.push(if i == 0 { 0 } else { 1 });
        entries.extend((*offset as u32).to_be_bytes());
        entries.extend([0, 0]);
    }
    write!(
        source,
        "5 0 obj\n<< /Type /XRef /Size 6 /Root 1 0 R /W [1 4 2] /Length {} >>\nstream\n",
        entries.len()
    )
    .unwrap();
    source.extend(entries);
    write!(source, "\nendstream\nendobj\nstartxref\n{xref}\n%%EOF\n").unwrap();
    source
}
#[test]
fn accepts_minimal_pdf2_and_uses_bottom_left_coordinates() {
    let mut d = Document::open(pdf(b"1 0 0 rg 0 0 50 50 re f", "", ""), Limits::default()).unwrap();
    let f = d.render(0, 1.).unwrap();
    let pixel = |x: usize, y: usize| &f.rgba[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4];
    assert_eq!(pixel(25, 75), [255, 0, 0, 255]);
    assert_eq!(pixel(25, 25), [255, 255, 255, 255]);
}
#[test]
fn rejects_unsupported_visual_semantics() {
    for extra in [
        "/Rotate 90",
        "/Annots []",
        "/UserUnit 2",
        "/Group << /S /Transparency >>",
    ] {
        assert!(
            Document::open(pdf(b"", extra, ""), Limits::default()).is_err(),
            "{extra}"
        );
    }
    for extra in ["/Rotate 90", "/CropBox [0 0 20 20]"] {
        assert!(Document::open(pdf(b"", "", extra), Limits::default()).is_err());
    }
}
#[test]
fn operators_fail_explicitly() {
    for content in [
        "sh",
        "BI",
        "1 0 0 0 k",
        "/Pattern cs",
        "/F 12 Tf BT (abc) Tj ET",
        "q",
        "Q",
        "BT",
        "ET",
        "1 w 10 10 m 20 20 l",
        "/Optional OC BDC EMC",
        "0 w",
        "0 M",
        "[0 0] 0 d",
        "[1] 0 cm",
        "1e300 0 0 1 0 0 cm",
    ] {
        let source = pdf(content.as_bytes(), "", "");
        let mut d = Document::open(source, Limits::default()).unwrap();
        assert!(d.render(0, 1.).is_err(), "accepted {content}");
    }
}
#[test]
fn composed_transform_and_operation_limits() {
    let mut d = Document::open(
        pdf(
            b"1000000000 0 0 1000000000 0 0 cm 1000000000 0 0 1000000000 0 0 cm",
            "",
            "",
        ),
        Limits::default(),
    )
    .unwrap();
    assert!(d.render(0, 1.).is_err());
    let limits = Limits {
        operations: 2,
        ..Default::default()
    };
    let mut d = Document::open(pdf(b"q Q q Q", "", ""), limits).unwrap();
    assert!(d.render(0, 1.).is_err());
}
#[test]
fn clips_and_restores_graphics_state() {
    let mut d = Document::open(
        pdf(
            b"q 0 0 50 50 re W n 1 0 0 rg 0 0 100 100 re f Q 0 0 1 rg 70 70 20 20 re f",
            "",
            "",
        ),
        Limits::default(),
    )
    .unwrap();
    let f = d.render(0, 1.).unwrap();
    assert_eq!(
        &f.rgba[(80 * 100 + 20) * 4..(80 * 100 + 20) * 4 + 4],
        [255, 0, 0, 255]
    );
    assert_eq!(
        &f.rgba[(20 * 100 + 80) * 4..(20 * 100 + 80) * 4 + 4],
        [0, 0, 255, 255]
    );
    assert_eq!(
        &f.rgba[(20 * 100 + 20) * 4..(20 * 100 + 20) * 4 + 4],
        [255, 255, 255, 255]
    );
}
#[test]
fn malformed_stream_and_xref_rejected() {
    let source = pdf(b"q Q", "", "");
    for needle in [b"/Length 3".as_slice(), b"/W [1 4 2]", b"/Root 1 0 R"] {
        let mut bad = source.clone();
        let at = bad.windows(needle.len()).position(|x| x == needle).unwrap();
        bad[at] = b'?';
        assert!(
            Document::open(bad, Limits::default())
                .and_then(|mut d| d.render(0, 1.))
                .is_err()
        );
    }
}
#[test]
fn truncated_eof_is_rejected() {
    let source = pdf(b"", "", "");
    for remove in 2..=10 {
        assert!(
            Document::open(source[..source.len() - remove].to_vec(), Limits::default()).is_err()
        );
    }
}
