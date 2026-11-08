use mata_docx::{Document,Length};
#[test]
fn can_open_deflate_compressed_ms_word_style_docx() {
    let data=include_bytes!("fixture_deflate.docx");
    let d=Document::from_bytes(data).unwrap();
    assert!(d.text().contains("DEFLATE interoperability fixture"));
    assert_eq!(d.properties.title,"Compressed fixture");
    let repacked=d.to_docx().unwrap();
    assert!(repacked.starts_with(b"PK\x03\x04"));
    let reread=Document::from_bytes(&repacked).unwrap();
    assert!(reread.text().contains("DEFLATE interoperability fixture"));
}
#[test]
fn images_headers_annotations_roundtrip() {
    let mut d=Document::new();
    d.header.push(mata_docx::Paragraph::new("Header"));
    d.footer.push(mata_docx::Paragraph::new("Footer"));
    d.add_paragraph("Main paragraph");
    let note=d.add_footnote("Footnote text");
    d.add_paragraph("Reference").add_footnote_ref(note);
    let comment=d.add_comment("Author","Comment text");
    d.add_paragraph("Commented").comment_range_start(comment).comment_range_end(comment);
    let img=d.add_image("tiny.png", include_bytes!("tiny.png").to_vec(),Length::inches(1.0),Length::inches(1.0));
    d.add_image_paragraph(img);
    let bytes=d.to_docx().unwrap();
    let reopened=Document::from_bytes(&bytes).unwrap();
    assert!(!reopened.images.is_empty());
    assert_eq!(reopened.footnotes[0].text,"Footnote text");
    assert_eq!(reopened.comments[0].text,"Comment text");
}
