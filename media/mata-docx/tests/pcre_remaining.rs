//! Source-only regression scenarios for the native Rust PCRE compatibility work.
//! The user will run these once the complete library is ready.
use mata_docx::pattern::{CalloutDecision,CalloutInfo,CalloutToken,Pattern};

#[test]
fn rust_callouts_have_no_ffi() {
    let pattern=Pattern::new(r"(?<word>a)(?C7)b(?C'checkpoint')").unwrap();
    let mut received=Vec::new();
    let caps=pattern.try_captures_from_with_callout("ab",0,&mut |callout:&CalloutInfo|{
        received.push(callout.token.clone());
        assert!(callout.current_byte<=2);
        CalloutDecision::Continue
    }).unwrap().unwrap();
    assert_eq!(caps.range,(0,2));
    assert_eq!(received,vec![CalloutToken::Number(7),CalloutToken::String("checkpoint".into())]);

    let error=pattern.try_captures_from_with_callout("ab",0,&mut |_|CalloutDecision::Abort);
    assert_eq!(error.unwrap_err().kind(),std::io::ErrorKind::Interrupted);
}

#[test]
fn scan_captured_substring_from_its_start() {
    let p=Pattern::new(r"(?<w>\w+)(*scs:(<w>).+rh)").unwrap();
    assert!(p.is_match("myrrh"));
    assert!(!p.is_match("apple"));
    assert!(Pattern::new(r"(?<n>a)(*scs:(<n>)^a)").unwrap().is_match("a"));
    assert!(Pattern::new(r"(*scs:(missing)x)").is_err());
}

#[test]
fn script_runs_reject_mixed_alphabet_spoofing() {
    let p=Pattern::new(r"^(*sr:\S+)$").unwrap();
    assert!(p.is_match("paypal.com"));
    assert!(!p.is_match("paypаl.com")); // Cyrillic 'а'
    assert!(p.is_match("日本かな")); // Han and Hiragana are a valid Japanese run
    let atomic=Pattern::new(r"^(*asr:\S+)$").unwrap();
    assert!(atomic.is_match("latin"));
}

#[test]
fn unicode_script_aliases_and_caret_negation() {
    assert!(Pattern::new(r"\p{sc:Cyrl}").unwrap().is_match("Ж"));
    assert!(Pattern::new(r"\p{scx:Latin}").unwrap().is_match("A"));
    assert!(Pattern::new(r"\p{^Latin}").unwrap().is_match("Ж"));
    assert!(!Pattern::new(r"\p{^Latin}").unwrap().is_match("A"));
    assert!(Pattern::new(r"\p{gc:Lu}").unwrap().is_match("A"));
}

#[test]
fn uts_18_extended_classes_are_opt_in() {
    let p=Pattern::new_with_alt_extended_classes(r"[\p{L}--[QW]]+").unwrap();
    assert_eq!(p.find("QWGK"),Some((2,4)));
    let p=Pattern::new_with_alt_extended_classes(r"[A-Z0-9&&[^E8]]+").unwrap();
    assert_eq!(p.find("E8AB"),Some((2,4)));
    let p=Pattern::new_with_alt_extended_classes(r"[\p{L}&&[\p{Greek}||\p{Latin}]]+").unwrap();
    assert_eq!(p.find("Дabc"),Some((2,5)));
    assert!(Pattern::new_with_alt_extended_classes(r"[[ab]&&[a]--[b]]").is_err());
}

#[test]
fn quoted_classes_and_numeric_escapes_are_decoded() {
    assert!(Pattern::new(r"[\x{41}-\x{5A}]").unwrap().is_match("Q"));
    assert!(Pattern::new(r"[\o{141}]").unwrap().is_match("a"));
    assert!(Pattern::new(r"[\Qa.]\E]").unwrap().is_match("."));
}

#[test]
fn empty_global_retries_before_advance() {
    let (text,count)=Pattern::new(r"(?=a)|a").unwrap().try_replace_all("a","X").unwrap();
    assert_eq!((text,count),("XX".into(),2));
}

#[test]
fn turkish_casing_is_explicit() {
    assert!(Pattern::new(r"(*TURKISH_CASING)(?i)I").unwrap().is_match("ı"));
    assert!(!Pattern::new(r"(*TURKISH_CASING)(?i)I").unwrap().is_match("i"));
    assert!(Pattern::new(r"(*TURKISH_CASING)(?i)İ").unwrap().is_match("i"));
}
