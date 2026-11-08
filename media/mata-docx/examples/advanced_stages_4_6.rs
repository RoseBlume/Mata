//! Stage 4–6 additions; running examples is deferred until your full build.
use mata_docx::{Document, ImageWrap, FloatingOptions, Length};
use mata_docx::pattern::Pattern;
use mata_docx::layout::Engine;

fn main()->Result<(), Box<dyn std::error::Error>> {
    let pattern=Pattern::new(r"(?<word>\p{L}+)(?>\s+)(?(word)ok|missing)")?;
    println!("atomic/conditional: {:?}",pattern.find("Hello ok"));
    let mut doc=Document::new();
    doc.add_paragraph("Tight polygon picture wrapping and note-aware pagination.");
    let note=doc.add_footnote("A footnote that reserves space before the line is positioned.");
    doc.add_paragraph("Reference").add_footnote_ref(note);
    // A valid image buffer can be loaded from disk and placed as follows.
    let png=std::fs::read("tests/tiny.png")?;
    let index=doc.add_image("tiny.png",png,Length::pt(120.0),Length::pt(80.0));
    let position=FloatingOptions{
        wrap:ImageWrap::Tight,
        wrap_polygon:vec![(10800,0),(21600,21600),(0,21600)],
        allow_overlap:false,
        ..FloatingOptions::default()
    };
    doc.images[index].float(position);
    doc.add_image_paragraph(index);
    let layout=doc.layout(&Engine::new())?;
    println!("{} pages",layout.pages.len());
    doc.save("advanced_stages_4_6.docx")?;
    Ok(())
}
