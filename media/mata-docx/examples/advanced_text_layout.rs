//! Example APIs added during the Stage 4–6 implementation pass.
use mata_docx::{Document, unicode, layout::Engine, hyphenation::Hyphenator};

fn main() -> std::io::Result<()> {
    let mut doc = Document::new();
    doc.add_paragraph("Internationalization makes multilingual documents easier to share.");
    doc.add_paragraph("Arabic: مرحبا بكم — mixed left-to-right text.");

    let mut engine = Engine::new();
    let mut hyphenator = Hyphenator::new();
    hyphenator.add_exception("in-ter-na-tion-al-iza-tion");
    // Applications may load a full licensed TeX pattern dictionary instead.
    engine.hyphenator = Some(hyphenator);

    let pages = doc.layout(&engine)?;
    println!("pages: {}", pages.pages.len());
    println!("graphemes: {}", unicode::graphemes("👩‍💻 hello").len());
    println!("line-break offsets: {:?}", unicode::line_breaks("Text 漢字 example"));
    println!("word boundaries: {:?}", unicode::word_boundaries("It's 12.50 today"));
    println!("sentence boundaries: {:?}", unicode::sentence_boundaries("Hello! How are you?"));
    Ok(())
}

// A host application owning an existing mata-fonts Ttf can pass it here.
#[allow(dead_code)]
fn rasterize_with_existing_font(font: &mut mata_fonts::decoders::ttf::Ttf) -> std::io::Result<usize> {
    let glyphs = mata_docx::mata_font_bridge::rasterize_text(font, "Hello", 12)?;
    Ok(glyphs.len())
}
