use mata_i18n::{args, Date, DateStyle, I18n, Locale, TextDirection, format_date, negotiate};

#[test]
fn resources_and_country_fallback() {
    let l10n = I18n::new("en-US").unwrap();
    let en = include_str!("../locales/en-US.lang");
    let fr = include_str!("../locales/fr.json");
    assert_eq!(l10n.add_resource("en-US", en).unwrap(), 6);
    assert_eq!(l10n.add_json("fr", fr).unwrap(), 6);
    l10n.set_locale("fr-CA").unwrap();
    assert_eq!(l10n.tr("actions.save"), "Enregistrer");
    assert_eq!(l10n.tr_with("app.file_count", &args! { "count" => 3 }), "3 fichiers");
    assert_eq!(l10n.tr_with("app.welcome", &args! { "name" => "Alix" }), "Bienvenue, Alix !");
    assert_eq!(l10n.tr_with("app.notification", &args! { "person" => "Alice" }), "Message de Alice");
    l10n.set_locale("ja-JP").unwrap();
    assert_eq!(l10n.tr("actions.save"), "Save");
    assert_eq!(l10n.locale().language(), "ja");
}

#[test]
fn errors_and_missing_variables() {
    let l10n = I18n::new("en").unwrap();
    l10n.add_resource("en", "intro = Hi {name}!").unwrap();
    assert!(l10n.translate("intro", &args! {}).is_err());
    assert!(l10n.translate("no.such.key", &args! {}).is_err());
    assert_eq!(l10n.tr_with("intro", &args! { "name" => "{count, plural}" }), "Hi {count, plural}!");
    assert!(l10n.add_json("en", r#"{"bad":"{broken"}"#).is_err());
    assert_eq!(l10n.tr_with("intro", &args! { "name" => "OK" }), "Hi OK!");
}

#[test]
fn metadata_and_date() {
    let requested = Locale::parse("ar-EG").unwrap();
    assert_eq!(requested.direction(), TextDirection::RightToLeft);
    let supported = [Locale::parse("en").unwrap(), Locale::parse("fr").unwrap()];
    assert_eq!(negotiate("fr-CA, en;q=0.8", &supported, &supported[0]), supported[1]);
    let d = Date::new(2000, 2, 29).unwrap();
    assert_eq!(format_date(&supported[1], d, DateStyle::Long), "29 février 2000");
}
