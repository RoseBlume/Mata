//! Regression scenarios for pure-Rust PCRE-style pattern behavior.
//! These tests are supplied for later local execution; they have not been run.
use mata_docx::pattern::Pattern;

#[test]
fn match_start_and_anchoring() {
    let p=Pattern::new(r"foo\Kbar").unwrap();
    assert_eq!(p.find("foobar"),Some((3,6)));
    let p=Pattern::new(r"\Gbar").unwrap();
    assert_eq!(p.try_captures_from("foobar",3).unwrap().unwrap().range,(3,6));
    assert_eq!(Pattern::new(r"bar\Z").unwrap().find("bar\n"),Some((0,3)));
    assert_eq!(Pattern::new(r"bar\z").unwrap().find("bar\n"),None);
}

#[test]
fn modes_apply_at_lexical_position() {
    assert_eq!(Pattern::new(r"a(?i)bc(?-i)d").unwrap().find("aBCd"),Some((0,4)));
    assert_eq!(Pattern::new(r"a(?i)bc(?-i)d").unwrap().find("aBCD"),None);
    assert!(Pattern::new("(?x) a  # ignored comment\n b").unwrap().is_match("ab"));
    assert_eq!(Pattern::new(r"(?U)a+?").unwrap().find("aaa"),Some((0,3)));
    assert_eq!(Pattern::new(r"(?U)a+").unwrap().find("aaa"),Some((0,1)));
}

#[test]
fn branch_reset_groups_and_backreferences() {
    let p=Pattern::new(r"(?|(a)|(b))(c)\g{1}").unwrap();
    assert!(p.is_match("aca"));
    assert!(p.is_match("bcb"));
    assert_eq!(p.captures("bcb").unwrap().group("bcb",2),Some("c"));
    let p=Pattern::new(r"(?<word>ab)\k'word'").unwrap();
    assert!(p.is_match("abab"));
    let p=Pattern::new(r"(ab)\g{-1}").unwrap();
    assert!(p.is_match("abab"));
}

#[test]
fn conditional_define_and_recursion() {
    assert!(Pattern::new(r"(a)?(?(1)b|c)").unwrap().is_match("ab"));
    assert!(Pattern::new(r"(a)?(?(1)b|c)").unwrap().is_match("c"));
    let p=Pattern::new(r"(?(DEFINE)(?<pair>ab))(?&pair)").unwrap();
    assert_eq!(p.find("ab"),Some((0,2)));
}

#[test]
fn verbs_and_bsr_modes() {
    let p=Pattern::new(r"foo(*SKIP)(*FAIL)|bar").unwrap();
    assert_eq!(p.find("foobar"),Some((3,6)));
    let caps=Pattern::new(r"(*MARK:tag)a(*ACCEPT)z").unwrap().captures("az").unwrap();
    assert_eq!(caps.range,(0,1));
    assert_eq!(caps.mark.as_deref(),Some("tag"));
    assert_eq!(Pattern::new(r"(*BSR_ANYCRLF)\R").unwrap().find("\u{85}"),None);
    assert_eq!(Pattern::new(r"(*BSR_UNICODE)\R").unwrap().find("\u{85}"),Some((0,2)));
}

#[test]
fn classes_and_escapes() {
    assert!(Pattern::new(r"[[:digit:]]+").unwrap().is_match("123"));
    assert!(!Pattern::new(r"[[:digit:]]+").unwrap().is_match("abc"));
    assert_eq!(Pattern::new(r"\N{U+0041}\o{102}").unwrap().find("AB"),Some((0,2)));
    assert!(Pattern::new(r"(*UCP)\d").unwrap().is_match("٣"));
    assert!(!Pattern::new(r"\d").unwrap().is_match("٣"));
    assert!(Pattern::new(r"\Q[a]+\E").unwrap().is_match("[a]+"));
}

#[test]
fn budgets_report_failure_not_absence() {
    let p=Pattern::new(r"(a|aa)+!$").unwrap().with_step_limit(30);
    assert!(p.try_captures_from(&"a".repeat(200),0).is_err());
}
