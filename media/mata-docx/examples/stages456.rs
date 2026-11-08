//! Stage 4–6 API demonstration. Run when you are ready to test all stages.
use mata_docx::Document;
use mata_docx::template::{Options, Value};
use mata_docx::{layout,unicode};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut doc=Document::new();
    doc.add_heading("Quote for {{customer.name}}",1);
    {
        let p=doc.add_paragraph("");
        p.add_run("Hello {{cust").bold(true);
        p.add_run("omer.name}} — your items:");
    }
    doc.add_paragraph("{{#each items}}");
    doc.add_paragraph("{{@index}}. {{this}} ");
    doc.add_paragraph("{{/each}}");
    doc.add_paragraph("{{#if approved}}");
    doc.add_paragraph("Status: approved");
    doc.add_paragraph("{{else}}");
    doc.add_paragraph("Status: pending");
    doc.add_paragraph("{{/if}}");
    let values=Value::object([
        ("customer".into(),Value::object([("name".into(),Value::from("Alex"))])),
        ("items".into(),Value::Array(vec![Value::from("Paper"),Value::from("Ink")])),
        ("approved".into(),Value::from(true)),
    ]);
    let report=doc.render_template(&values,&Options::default())?;
    let count=doc.replace_regex_across_runs(r"(?i)(paper|ink)","[$1]")?;
    let pages=doc.layout(&layout::Engine::new())?;
    println!("{} substitutions, {} regex replacements, {} layout pages",report.substitutions,count,pages.pages.len());
    println!("Unicode NFC: {}",unicode::nfc("Cafe\u{0301}"));
    doc.save("stages456.docx")?;
    Ok(())
}
