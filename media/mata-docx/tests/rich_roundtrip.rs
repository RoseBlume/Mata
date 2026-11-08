use mata_docx::{Block,RichDocument,Inline,Run,RichParagraph,decode_rich,encode_rich};

#[test]
fn imports_and_rewrites_a_word_table(){
    let source=include_bytes!("fixtures/word_created.docx");
    let mut doc=decode_rich(source).expect("read Word fixture");
    let table_pos=doc.blocks.iter().position(|b|matches!(b,Block::Table(_))).expect("table in Word fixture");
    doc.set_cell_text(table_pos,0,0,"Revised cell ✓").expect("edit cell");
    let bytes=encode_rich(&doc).expect("write structured DOCX");
    let restored=decode_rich(&bytes).expect("read structured DOCX");
    assert!(restored.to_plain_text().contains("Revised cell ✓"));
    assert!(matches!(&restored.blocks[table_pos],Block::Table(_)));
}
#[test]
fn citations_and_bookmarks_are_editable(){
    let mut d=RichDocument::new();
    d.insert_bookmark(0,"Anchor2026").unwrap();
    d.insert_cross_reference(1,"Anchor2026","see previous section").unwrap();
    d.insert_citation(2,"Book2026","(Author, 2026)").unwrap();
    d.blocks.push(Block::Paragraph(RichParagraph{runs:vec![Inline::Text(Run{text:"Description".into(),..Run::default()})],..RichParagraph::default()}));
    let encoded=encode_rich(&d).unwrap();let copy=decode_rich(&encoded).unwrap();
    assert!(copy.to_plain_text().contains("(Author, 2026)"));
}
