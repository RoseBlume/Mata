//! Source-only illustration of newly added stage 4–6 APIs.
use mata_docx::{Document, VerticalAlignment};
use mata_docx::pattern::Pattern;
use mata_docx::layout::Engine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pattern = Pattern::new(r"(?i:\X+)\R")?;
    let (changed, count) = pattern.try_replace_all("Hello\r\nWORLD\n", "[$&]")?;
    println!("{count} replacements: {changed:?}");

    let mut doc = Document::new();
    let paragraph = doc.add_paragraph("Keep this paragraph on one page if it fits.");
    paragraph.keep_together = true;
    let table = doc.add_table(2, 2);
    table.rows[0].cells[0].valign = VerticalAlignment::Center;
    table.rows[1].cells[1].valign = VerticalAlignment::Bottom;

    let pages = Engine::new().paginate(&doc)?;
    println!("{} pages", pages.pages.len());
    Ok(())
}
