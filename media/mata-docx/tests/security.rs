use mata_docx::{package::{Package,resolve_target},xml,zip};
use std::collections::BTreeMap;

#[test]fn malformed_archive_fixtures_are_rejected(){
    for bytes in [
        &include_bytes!("security_cases/zip_traversal.zip")[..],
        &include_bytes!("security_cases/zip_duplicate.zip")[..],
        &include_bytes!("security_cases/zip_truncated.zip")[..],
    ] {assert!(zip::decode(bytes).is_err());}
}
#[test]fn xml_entity_and_namespace_attacks_are_rejected(){
    for xml_doc in [
        include_str!("security_cases/evil_doctype.xml"),
        include_str!("security_cases/broken_namespace.xml"),
        include_str!("security_cases/duplicate_attribute.xml"),
    ] {assert!(xml::parse(xml_doc).is_err());}
}
#[test]fn zip64_directory_supports_many_entries(){
    let mut parts=BTreeMap::new();
    for i in 0..65_536 {parts.insert(format!("item/{i:05}.xml"),b"test".to_vec());}
    let bytes=zip::encode(&parts).unwrap();
    assert!(bytes.windows(4).any(|p|p==b"PK\x06\x06"));
    assert_eq!(zip::decode(&bytes).unwrap(),parts);
}
#[test]fn fixed_deflate_writer_interoperates_with_zip_reader(){
    let mut files=BTreeMap::new();
    files.insert("word/document.xml".to_owned(),"Repetition! ".repeat(4096).into_bytes());
    let data=zip::encode(&files).unwrap();
    assert_eq!(u16::from_le_bytes([data[8],data[9]]),8); // method 8, raw DEFLATE
    assert!(data.len()<5000);
    assert_eq!(zip::decode(&data).unwrap(),files);
}
#[test]fn opc_target_resolution_is_bounded(){
    for target in ["../../other", "%2e%2e/%2e%2e/file", "http://x/a", "bad%2fpath"]{
        assert!(resolve_target("word/document.xml",target).is_err());
    }
    assert_eq!(resolve_target("word/document.xml","../customXml/item1.xml#frag").unwrap(),"customXml/item1.xml");
}
#[test]fn package_preserves_unknown_parts(){
    let fixture=include_bytes!("fixture_deflate.docx");
    let package=Package::from_bytes(fixture).unwrap();
    let roundtrip=Package::from_bytes(&package.to_bytes().unwrap()).unwrap();
    assert_eq!(package.parts,roundtrip.parts);
}
/// Deterministic dependency-free mutation fuzzing: a bounded corpus of inputs
/// must yield either a valid result or an error, never panic or hang.
#[test]fn mutation_fuzz_parser_does_not_panic(){
    let seed=include_bytes!("fixture_deflate.docx");
    let mut rng=0x9e3779b97f4a7c15u64;
    let next=|rng:&mut u64| {let mut s=*rng;s^=s<<13;s^=s>>7;s^=s<<17;*rng=s;s};
    for _ in 0..3000 {
        let mut corrupted=seed.to_vec();
        for _ in 0..3 {let ix=(next(&mut rng) as usize)%corrupted.len();corrupted[ix]^=(1+(next(&mut rng)%255)) as u8;}
        assert!(std::panic::catch_unwind(||zip::decode(&corrupted)).is_ok());
    }
    for _ in 0..3000 {
        let mut contents=b"<root xmlns:w='urn:test'><w:child>hello</w:child></root>".to_vec();
        for _ in 0..2 {let ix=(next(&mut rng) as usize)%contents.len();contents[ix]=(next(&mut rng)%128) as u8;}
        if let Ok(text)=std::str::from_utf8(&contents){assert!(std::panic::catch_unwind(||xml::parse(text)).is_ok());}
    }
}

#[test]fn modified_package_signatures_are_invalidated(){
    let mut pkg=Package::from_bytes(include_bytes!("fixture_deflate.docx")).unwrap();
    pkg.parts.insert("_xmlsignatures/sig1.xml".into(),b"<Signature/>".to_vec());
    let rels=String::from_utf8(pkg.parts["_rels/.rels"].clone()).unwrap();
    let rels=rels.replace("</Relationships>","<Relationship Id=\"rId100\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/origin\" Target=\"_xmlsignatures/origin.sigs\"/></Relationships>");
    pkg.parts.insert("_rels/.rels".into(),rels.into_bytes());
    assert_eq!(pkg.signed_parts().len(),1);
    pkg.invalidate_signatures();
    assert!(pkg.signed_parts().is_empty());
    assert!(!String::from_utf8_lossy(&pkg.parts["_rels/.rels"]).contains("digital-signature"));
}
