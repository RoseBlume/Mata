//! `mata-docx`: a Rust OOXML implementation using `std` plus local `mata-fonts`.
//! Supports reading the OOXML Word document format: DEFLATE DOCX, authoring
//! DOCX, styled paragraphs, tables, links, images, notes and multiple exports.
//!
//! ```no_run
//! use mata_docx::{Document, Length};
//! let mut doc = Document::new();
//! doc.add_heading("Hello from mata-docx", 1);
//! doc.add_paragraph("A Word document without third-party crates.");
//! doc.add_table(2, 3).cell_mut(0, 0).unwrap().set_text("Item");
//! doc.save("example.docx").unwrap();
//! ```
#![forbid(unsafe_code)]
pub mod zip;
pub mod package;
pub mod xml;
pub mod stream;
pub mod fonts;
pub mod model;
mod casefold_data;
mod pcre_script_data;
pub mod pattern;
pub mod template;
pub mod unicode;
mod ucd_textbreak;
mod arabic_data;
mod indic;
mod ucd_regex;
pub mod textbreak;
pub mod hyphenation;
mod variations;
pub mod glyph_variations;
pub mod gdef;
pub mod colr;
pub mod bitmap_fonts;
pub mod ebdt;
pub mod svg_color;
pub mod truetype_hinting;
mod colr_v1;
mod cff2;
pub mod opentype;
pub mod mata_font_bridge;
pub mod layout;
mod ucd_tables;
mod ooxml;
mod export;
pub use model::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn docx_reopen(){
        let mut d=Document::new();d.properties.title="Unit test".to_owned();
        d.add_heading("A heading",1);d.add_paragraph("Hello world").add_run("! 🦀").bold(true);
        d.add_paragraph("").add_hyperlink("https://example.com","example");
        d.add_table(2,2).cell_mut(0,0).unwrap().set_text("cell");
        let bytes=d.to_docx().unwrap();let decoded=Document::from_bytes(&bytes).unwrap();
        assert!(decoded.text().contains("Hello world"));assert!(decoded.text().contains("cell"));
        assert_eq!(decoded.properties.title,"Unit test");
    }
    #[test]fn formats(){let mut d=Document::new();d.add_paragraph("Hello Rust");assert!(d.to_html().contains("Hello Rust"));assert!(d.to_markdown().contains("Hello Rust"));assert!(d.to_pdf().unwrap().starts_with(b"%PDF-"));assert!(d.to_png_pages().unwrap()[0].starts_with(b"\x89PNG"));}
}
