use mata_docx::Document;
use mata_docx::{pattern::Pattern,layout::Engine};

fn main() -> std::io::Result<()> {
    // A recursive named subroutine can match balanced parentheses.
    let nested=Pattern::new(r"(?<nested>\((?:[^()]|(?&nested))*\))")?;
    let text="Before (a(b(c))d) after";
    let result=nested.captures(text).expect("matching nested expression");
    assert_eq!(result.group(text,0),Some("(a(b(c))d)"));

    // Unicode default full folding handles scalar expansions such as ß -> ss.
    let folded=mata_docx::pattern::full_case_fold("Straße");
    assert_eq!(folded,"strasse");

    let mut doc=Document::new();
    doc.properties.title="Stage 4–6 example".into();
    doc.properties.creator="mata-docx".into();
    doc.add_heading("A shaped document",1);
    doc.add_paragraph("Writing systems: العربية Ελληνικά हिन्दी தமிழ் 日本語");
    doc.add_paragraph("Page ").add_page_number().add_run(" of ");
    doc.blocks.last_mut().and_then(|b|match b{
        mata_docx::Block::Paragraph(p)=>Some(p),_=>None
    }).unwrap().add_total_pages();
    let layout=Engine::new();
    // Register real OpenType faces with layout.fonts.add(...) in the host app.
    // Variable faces expose font.set_variation(*b"wght", 600.0).
    let pages=layout.paginate(&doc)?;
    println!("{} pages ({} warnings)",pages.pages.len(),pages.warnings.len());
    Ok(())
}
