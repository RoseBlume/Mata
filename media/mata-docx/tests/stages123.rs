use mata_docx::{Document,Paragraph,Length,MathExpr,EmbeddedFont,SectionBreak};
use mata_docx::package::Package;

#[test]fn styled_xml_and_unknown_body_preserved_if_untouched(){
    let d=Document::from_bytes(include_bytes!("fixture_deflate.docx")).unwrap();
    let original=Package::from_bytes(include_bytes!("fixture_deflate.docx")).unwrap();
    let written=Package::from_bytes(&d.to_docx().unwrap()).unwrap();
    assert_eq!(original.parts,written.parts);
}
#[test]fn sections_equations_and_threaded_comments_round_trip(){
    let mut d=Document::new();
    d.add_paragraph("Before section break");
    let section=d.add_section_break(SectionBreak::NextPage);
    section.columns=2;
    section.page_number_start=Some(7);
    d.add_paragraph("After section break")
        .add_math_expression(MathExpr::Fraction(Box::new(MathExpr::Text("x+1".into())),Box::new(MathExpr::Text("2".into()))));
    let parent=d.add_comment("Reviewer","Question");
    d.add_comment_reply(parent,"Editor","Answer").unwrap();
    let bytes=d.to_docx().unwrap();
    let pkg=Package::from_bytes(&bytes).unwrap();pkg.validate().unwrap();
    let document=std::str::from_utf8(pkg.parts.get("word/document.xml").unwrap()).unwrap();
    assert!(document.contains("<m:f>"));
    assert!(document.contains("w:num=\"2\""));
    assert!(document.contains("w:start=\"7\""));
    let loaded=Document::from_bytes(&bytes).unwrap();
    assert_eq!(loaded.sections[0].columns,2);
    assert_eq!(loaded.comments[1].parent,Some(parent));
}
#[test]fn revision_accept_reject(){
    let mut d=Document::new();
    let p=d.add_paragraph("A");p.add_tracked_insert("M","B");p.add_tracked_delete("M","C");
    let bytes=d.to_docx().unwrap();
    let loaded=Document::from_bytes(&bytes).unwrap();
    let mut accept=loaded.clone();accept.accept_revisions();
    assert_eq!(accept.text(),"AB");
    let mut reject=loaded;reject.reject_revisions();
    assert_eq!(reject.text(),"AC");
}
#[test]fn glossary_item_properties_and_font_parts_are_wired(){
    let mut d=Document::new();d.add_paragraph("Document");
    d.add_building_block("AddressBlock","General",vec![Paragraph::new("123 Main St")]);
    d.add_custom_xml("item1.xml","<data>ok</data>");
    d.add_custom_xml_item_props("item1.xml","<ds:datastoreItem xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\" ds:itemID=\"{00000000-0000-0000-0000-000000000001}\"/>");
    d.add_embedded_font(EmbeddedFont{name:"Example Obfuscated Font".into(),obfuscated_data:vec![0u8;48],key_guid:"{00000000-0000-0000-0000-000000000002}".into()});
    let pkg=Package::from_bytes(&d.to_docx().unwrap()).unwrap();
    pkg.validate().unwrap();
    assert!(pkg.parts.contains_key("word/glossary/document.xml"));
    assert!(pkg.parts.contains_key("word/fontTable.xml"));
    assert!(pkg.parts.contains_key("customXml/itemProps1.xml"));
    let reloaded=Document::from_bytes(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!(reloaded.glossary.len(),1);
    assert_eq!(reloaded.embedded_fonts.len(),1);
    assert_eq!(reloaded.custom_xml_item_props.len(),1);
}
#[test]fn floating_images_author_anchor_xml(){
    let mut d=Document::new();
    let ix=d.add_image("tiny.png",include_bytes!("tiny.png").to_vec(),Length::inches(1.0),Length::inches(1.0));
    d.images[ix].floating=true;
    d.add_image_paragraph(ix);
    let pkg=Package::from_bytes(&d.to_docx().unwrap()).unwrap();
    let part=std::str::from_utf8(pkg.parts.get("word/document.xml").unwrap()).unwrap();
    assert!(part.contains("<wp:anchor"));
    assert!(!part.contains("<wp:inline"));
}

#[test]fn unknown_top_level_xml_survives_edit(){
    let mut pkg=Package::from_bytes(include_bytes!("fixture_deflate.docx")).unwrap();
    let xml=String::from_utf8(pkg.parts["word/document.xml"].clone()).unwrap();
    let changed=xml.replace("<w:sectPr>","<w:sdt><w:sdtContent><w:p><w:r><w:t>unmodeled</w:t></w:r></w:p></w:sdtContent></w:sdt><w:sectPr>");
    pkg.parts.insert("word/document.xml".to_owned(),changed.into_bytes());
    let mut document=Document::from_bytes(&pkg.to_bytes().unwrap()).unwrap();
    document.replace_text("DEFLATE", "Modified");
    let rewritten=Package::from_bytes(&document.to_docx().unwrap()).unwrap();
    let xml=String::from_utf8(rewritten.parts["word/document.xml"].clone()).unwrap();
    assert!(xml.contains("<w:sdt>"));
    assert!(xml.contains("Modified interoperability fixture"));
}
#[test]fn table_properties_survive_round_trip(){
    let mut doc=Document::new();
    let table=doc.add_table(1,1);
    table.layout_fixed=true;
    table.alignment=mata_docx::Alignment::Center;
    table.indent_twips=Some(600);
    table.cell_margin_twips=Some(80);
    table.rows[0].cant_split=true;
    table.cell_mut(0,0).unwrap().text_direction=Some("tbRl".to_string());
    let imported=Document::from_bytes(&doc.to_docx().unwrap()).unwrap();
    let table=imported.tables().next().unwrap();
    assert!(table.layout_fixed);
    assert_eq!(table.alignment,mata_docx::Alignment::Center);
    assert_eq!(table.cell_margin_twips,Some(80));
    assert!(table.rows[0].cant_split);
    assert_eq!(table.cell(0,0).unwrap().text_direction.as_deref(),Some("tbRl"));
}
