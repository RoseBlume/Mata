use mata_docx::{Alignment,Document,Length,Protection,Style};
use std::io;
fn main()->io::Result<()>{
    let mut doc=Document::new();
    doc.properties.title="Mata DOCX Feature Showcase".to_owned();
    doc.properties.creator="mata-docx".to_owned();
    doc.header.push(mata_docx::Paragraph::new("Internal memo"));
    doc.footer.push(mata_docx::Paragraph::new("Generated with Rust std only"));
    doc.add_heading("Quarterly Project Report",1).alignment(Alignment::Center);
    doc.add_paragraph("This is a standard DOCX generated entirely in Rust without external dependencies.");
    doc.add_paragraph("Mixed formatting: ").add_run("bold and italic").bold(true).italic(true).color("0B5394");
    doc.add_heading("Progress",2).add_bookmark("progress");
    let t=doc.add_table(3,3);t.rows[0].is_header=true;
    for (j,text) in ["Workstream","Status","Owner"].iter().enumerate(){t.cell_mut(0,j).unwrap().set_text(*text).shade("D8E8FF");}
    for (j,text) in ["Core model","Ready","Team"].iter().enumerate(){t.cell_mut(1,j).unwrap().set_text(*text);}
    for (j,text) in ["Export","Basic","Team"].iter().enumerate(){t.cell_mut(2,j).unwrap().set_text(*text);}
    doc.add_paragraph("").add_hyperlink("https://www.rust-lang.org","Rust language website");
    doc.add_paragraph("").add_internal_link("progress","Go back to progress");
    let note=doc.add_footnote("This note demonstrates a real OOXML footnote part.");
    doc.add_paragraph("Footnote example").add_footnote_ref(note);
    let comment=doc.add_comment("Reviewer","Please review the wording.");
    doc.add_paragraph("Annotated paragraph").comment_range_start(comment).add_run(" with comment target");
    doc.add_paragraph("Form field: ").add_text_field("customer_name","Example customer");
    doc.add_paragraph("Approved: ").add_checkbox("approved",true);
    doc.add_paragraph("Status: ").add_dropdown("status",vec!["Draft".into(),"Approved".into()],1);
    doc.add_style(Style::paragraph("CustomReport","Custom Report"));
    doc.set_text_watermark("DRAFT");
    doc.page.margin_left=Length::inches(0.8).twips() as u32;
    doc.protect(Protection::CommentsOnly);
    doc.save("showcase.docx")?;
    doc.save_html("showcase.html")?;
    doc.save_markdown("showcase.md")?;
    doc.save_pdf("showcase.pdf")?;
    doc.save_png("showcase.png")?;
    Ok(())
}
