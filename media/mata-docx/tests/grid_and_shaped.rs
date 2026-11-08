use mata_docx::{Document,DocumentGrid,layout::Engine};

#[test]
fn paragraph_grid_roundtrip(){
    let mut doc=Document::new();
    doc.final_section_mut().doc_grid=Some(DocumentGrid{
        kind:"lines".into(),line_pitch_twips:Some(360),char_space:None,
    });
    let p=doc.add_paragraph("Paragraph outside the grid");
    p.snap_to_grid=Some(false);
    let bytes=doc.to_docx().expect("write DOCX");
    let read=Document::from_bytes(&bytes).expect("read DOCX");
    assert_eq!(read.final_section.as_ref().unwrap().doc_grid.as_ref().unwrap().line_pitch_twips,Some(360));
    let first=match &read.blocks[0]{mata_docx::Block::Paragraph(p)=>p,_=>panic!("expected paragraph")};
    assert_eq!(first.snap_to_grid,Some(false));
}

#[test]
fn baseline_grid_changes_line_advance(){
    let mut doc=Document::new();
    doc.add_paragraph("one\ntwo\nthree");
    let natural=Engine::new().paginate(&doc).expect("layout");
    doc.final_section_mut().doc_grid=Some(DocumentGrid{
        kind:"lines".into(),line_pitch_twips:Some(600),char_space:None,
    });
    let snapped=Engine::new().paginate(&doc).expect("layout");
    let natural_height=natural.pages[0].text.iter().map(|t|t.region.bottom()).fold(0.0,f32::max);
    let snapped_height=snapped.pages[0].text.iter().map(|t|t.region.bottom()).fold(0.0,f32::max);
    assert!(snapped_height>=natural_height);
}
