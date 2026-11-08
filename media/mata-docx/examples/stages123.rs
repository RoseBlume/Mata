use mata_docx::{Document,Paragraph,MathExpr,SectionBreak};
fn main()->std::io::Result<()> {
    let mut doc=Document::new();
    doc.add_heading("Stages 1–3: DOCX example",1);
    doc.add_paragraph("This example uses only the Rust standard library.");
    doc.footer.push(Paragraph::new("Page "));
    doc.footer.last_mut().unwrap().add_page_number();
    doc.add_section_break(SectionBreak::NextPage).page_number_start=Some(1);
    doc.add_paragraph("An OMML fraction: ").add_math_expression(MathExpr::Fraction(
        Box::new(MathExpr::Text("1".to_owned())),Box::new(MathExpr::Text("2".to_owned()))));
    doc.add_building_block("Legal Address","Reusable",vec![Paragraph::new("City, Country")]);
    doc.save("stages123.docx")
}
