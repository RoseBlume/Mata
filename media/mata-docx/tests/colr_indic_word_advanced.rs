//! Regression cases for the pure-Rust color-font, shaping and table layout pass.
//! These are intentionally not executed during source-authoring.
use mata_docx::{Document,RowHeightRule,layout::Engine};

#[test]
fn advanced_table_properties_roundtrip(){
    let mut doc=Document::new();
    let table=doc.add_table(3,4);
    table.width_percent=Some(72.5);
    table.rows[0].height_twips=Some(360);
    table.rows[0].height_rule=RowHeightRule::Exact;
    table.rows[1].height_twips=Some(260);
    table.rows[1].height_rule=RowHeightRule::AtLeast;
    table.rows[2].grid_before=1;
    table.rows[2].grid_after=2;
    let bytes=doc.to_docx().expect("DOCX generated");
    let read=Document::from_bytes(&bytes).expect("DOCX read");
    let result=match &read.blocks[0]{mata_docx::Block::Table(t)=>t,_=>panic!("expected table")};
    assert_eq!(result.width_percent,Some(72.5));
    assert_eq!(result.rows[0].height_rule,RowHeightRule::Exact);
    assert_eq!(result.rows[1].height_rule,RowHeightRule::AtLeast);
    assert_eq!(result.rows[2].grid_before,1);
    assert_eq!(result.rows[2].grid_after,2);
}

#[test]
fn exact_row_height_stays_exact(){
    let mut doc=Document::new();
    let table=doc.add_table(1,1);
    table.rows[0].height_twips=Some(360);
    table.rows[0].height_rule=RowHeightRule::Exact;
    table.cell_mut(0,0).unwrap().set_text("A long paragraph\nSecond line\nThird line");
    let layout=Engine::new().paginate(&doc).expect("pagination");
    assert!(layout.pages.iter().flat_map(|p|&p.cells)
        .any(|cell|(cell.region.height-18.).abs()<1.));
}

#[test]
fn percentage_table_width_affects_columns(){
    let mut doc=Document::new();
    let t=doc.add_table(1,2);t.width_percent=Some(50.);
    let result=Engine::new().paginate(&doc).expect("pagination");
    let cells=&result.pages[0].cells;
    assert_eq!(cells.len(),2);
    assert!((cells[0].region.width+cells[1].region.width) < 400.);
}
