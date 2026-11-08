use mata_i18n::{args, format_currency, format_date, Date, DateStyle, I18n};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let i18n = I18n::new("en-US")?;
    i18n.load_directory(concat!(env!("CARGO_MANIFEST_DIR"), "/locales"))?;
    for locale in ["en-US", "fr-CA"] {
        i18n.set_locale(locale)?;
        println!("{}", i18n.tr("app.title"));
        println!("{}", i18n.tr_with("app.welcome", &args! { "name" => "Alex" }));
        println!("{}", i18n.tr_with("app.file_count", &args! { "count" => 1234 }));
        println!("{}", format_currency(&i18n.locale(), 1999.99, "CAD"));
        println!("{}", format_date(&i18n.locale(), Date::new(2026, 10, 20).unwrap(), DateStyle::Long));
    }
    Ok(())
}
