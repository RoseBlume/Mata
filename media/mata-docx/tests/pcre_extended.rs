//! Regression scenarios prepared for later compilation/execution.
//! These tests have intentionally NOT been run in this environment.
use mata_docx::pattern::Pattern;

#[test]
fn duplicate_names_and_named_backreferences() {
    let pattern=Pattern::new(r"(?J)(?<letter>a)|(?<letter>b)").unwrap();
    let c=pattern.captures("b").unwrap();
    assert_eq!(c.named("b","letter"),Some("b"));
    assert!(Pattern::new(r"(?<x>a)(?<x>b)").is_err());
    assert!(Pattern::new(r"(?J)(?<x>a)?(?<x>b)\k<x>").unwrap().is_match("bb"));
}

#[test]
fn regex_conditionals_and_recursion_conditions() {
    assert_eq!(Pattern::new(r"(?(?=a)a|b)").unwrap().find("b"),Some((0,1)));
    assert_eq!(Pattern::new(r"(?(?!a)b|a)").unwrap().find("b"),Some((0,1)));
    assert!(Pattern::new(r"(a)?(?(1)b|c)").unwrap().is_match("ab"));
    assert!(Pattern::new(r"(a)?(?(1)b|c)").unwrap().is_match("c"));
}

#[test]
fn non_atomic_lookaround_finds_second_capture() {
    let subject="abc";
    assert_eq!(Pattern::new(r"(?=(a|ab))\1c").unwrap().find(subject),None);
    assert_eq!(Pattern::new(r"(?*(a|ab))\1c").unwrap().find(subject),Some((0,3)));
    assert_eq!(Pattern::new(r"(*napla:(a|ab))\1c").unwrap().find(subject),Some((0,3)));
}

#[test]
fn extended_class_operators() {
    let consonant=Pattern::new(r"(?[[a-z] - [aeiou]])+").unwrap();
    assert_eq!(consonant.find("aeiouxyz"),Some((5,8)));
    let digit=Pattern::new(r"(?[[0-9] & [2-7]])").unwrap();
    assert_eq!(digit.find("12345"),Some((1,2)));
    assert_eq!(digit.find("8"),None);
    let symmetric=Pattern::new(r"(?[[ab] ^ [bc]])").unwrap();
    assert_eq!(symmetric.find("bca"),Some((1,2)));
}

#[test]
fn backtracking_cut_operators() {
    assert_eq!(Pattern::new(r"foo(*SKIP)(*FAIL)|bar").unwrap().find("foobar"),Some((3,6)));
    assert_eq!(Pattern::new(r"abc(*COMMIT)d|abcX").unwrap().find("abcX"),None);
    assert_eq!(Pattern::new(r"abc(*PRUNE)d|abcX").unwrap().find("abcX"),None);
    assert_eq!(Pattern::new(r"ab(*THEN)c|abX").unwrap().find("abX"),Some((0,3)));
    assert_eq!(Pattern::new(r"a(*MARK:A)b(*SKIP:A)(*FAIL)|b").unwrap().find("ab"),Some((1,2)));
}

#[test]
fn newline_and_match_control_directives() {
    assert_eq!(Pattern::new(r"(*CR)bar$").unwrap().find("bar\r"),Some((0,3)));
    assert_eq!(Pattern::new(r"(*CRLF)bar\Z").unwrap().find("bar\r\n"),Some((0,3)));
    assert_eq!(Pattern::new(r"(*ANY)bar$").unwrap().find("bar\u{85}"),Some((0,3)));
    assert_eq!(Pattern::new(r"(*ANCHORED)bar").unwrap().find("foobar"),None);
    assert_eq!(Pattern::new(r"(*NOTEMPTY)a*").unwrap().find("bbb"),None);
    assert_eq!(Pattern::new(r"(*NOTEMPTY)a*").unwrap().find("aaa"),Some((0,3)));
}

#[test]
fn ascii_option_families() {
    assert!(Pattern::new(r"(*UCP)\d").unwrap().is_match("٣"));
    assert!(!Pattern::new(r"(*UCP)(?aD)\d").unwrap().is_match("٣"));
    assert!(Pattern::new(r"(?i)AbC").unwrap().is_match("abc"));
}

#[test]
fn pcre_style_substitution() {
    let subject="abc";
    let caps=Pattern::new(r"(?<letter>b)").unwrap().captures(subject).unwrap();
    assert_eq!(caps.expand(subject,r"$<letter>:$`:$':$_"),"b:a:c:abc");
    assert_eq!(caps.expand_extended(subject,r"${missing:-X}${letter:+YES:NO}"),"XYES");
    assert_eq!(caps.expand_extended(subject,r"\U$<letter>\E"),"B");
}
