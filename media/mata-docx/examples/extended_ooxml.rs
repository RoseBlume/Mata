//! Demonstrates new authoring APIs. Do not interpret this example as a test.
use mata_docx::{
    ContentBinding,Document, FloatingOptions, ImageWrap, Length, MathExpr,
    RevisionInfo, RevisionKind, SectionBreak, StructuredContentControl,
};
fn main()->std::io::Result<()> {
    let mut doc=Document::new();
    doc.properties.title="Expanded std-only OOXML".into();
    doc.add_heading("Document model",1);
    doc.add_paragraph("An example of a standard-library-only DOCX crate.");
    let section=doc.add_section_break(SectionBreak::NextPage);
    section.columns=2;section.column_spacing_twips=540;
    section.page_number_start=Some(2);
    section.page_number_format=Some("lowerRoman".into());
    let p=doc.add_paragraph("A revision: ");
    p.add_revision(RevisionKind::Insert,RevisionInfo{
        id:101,author:"Editor".into(),date:Some("2026-11-03T12:00:00Z".into()),
    },vec![mata_docx::Inline::Run(mata_docx::Run::new("Inserted text"))]);
    doc.add_paragraph("").add_math_expression(MathExpr::Matrix(vec![
        vec![MathExpr::Text("a".into()),MathExpr::Text("b".into())],
        vec![MathExpr::Text("c".into()),MathExpr::Text("d".into())],
    ]));
    doc.add_paragraph("").add_complex_field("DATE \\@ \"yyyy-MM-dd\"","2026-11-03");
    doc.add_paragraph("").add_structured_control(StructuredContentControl{
        tag:"ClientName".into(),alias:"Client name".into(),value:"Example".into(),
        binding:Some(ContentBinding{
            store_item_id:"{00000000-0000-0000-0000-000000000001}".into(),
            xpath:"/client/name".into(),prefix_mappings:String::new(),
        }),..Default::default()
    });
    let image=doc.add_image("logo.png",std::fs::read("tests/tiny.png")?,Length::inches(1.5),Length::inches(1.5));
    let options=FloatingOptions{wrap:ImageWrap::TopAndBottom,x_emu:914400,..Default::default()};
    doc.images[image].float(options);
    doc.add_image_paragraph(image);
    doc.save("extended-ooxml.docx")
}
