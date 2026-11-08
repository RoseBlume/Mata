//! Demonstrates WordprocessingML pagination controls with no native ABI.
use std::io;
use mata_docx::{Document,Section,SectionBreak,LineSpacingRule,TabStop,TabAlignment,TabLeader};
fn main()->io::Result<()> {
    let mut doc=Document::new();
    let page=doc.page.clone();
    let mut section=Section::new(usize::MAX,SectionBreak::NextPage,page);
    section.mirror_margins=true;
    section.gutter_twips=360;
    section.page_number_format=Some("lowerRoman".to_string());
    doc.final_section=Some(section);
    {
        let p=doc.add_paragraph("Summary\t18");
        p.keep_with_next=true;
        p.contextual_spacing=true;
        p.tab_stops.push(TabStop{position_twips:8500,alignment:TabAlignment::Right,leader:TabLeader::Dots});
    }
    {
        let p=doc.add_paragraph("The first paragraph has Word-style line rules and a right-aligned tab.");
        p.line_spacing=Some(280);
        p.line_rule=Some(LineSpacingRule::Auto);
        p.widow_control=Some(true);
    }
    let mut engine=mata_docx::layout::Engine::new();
    // Add font data to engine.fonts before pagination for accurate metrics.
    let output=engine.paginate(&doc)?;
    println!("{} pages",output.pages.len());
    doc.save("word-flow.docx")?;
    Ok(())
}
