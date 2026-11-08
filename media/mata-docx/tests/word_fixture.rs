use mata_docx::{decode,encode};
#[test]
fn imports_a_real_word_generated_docx(){
    let data=include_bytes!("fixtures/word_created.docx");
    let parsed=decode(data).unwrap();
    let text=parsed.paragraphs.iter().flat_map(|p|p.runs.iter()).map(|r|r.text.as_str()).collect::<String>();
    assert!(text.contains("Mata DOCX test"));
    assert!(text.contains("bold"));
    assert!(parsed.warnings.iter().any(|s|s.contains("Tables")));
    let generated=encode(&parsed).unwrap();
    assert!(decode(&generated).is_ok());
}
#[test]
fn damaged_zip_is_rejected(){let data=include_bytes!("fixtures/word_created.docx");assert!(decode(&data[..10]).is_err());}
