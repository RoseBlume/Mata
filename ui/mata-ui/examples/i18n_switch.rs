//! A tiny Mata UI app with runtime language switching.
//! cargo run -p mata-ui --example i18n_switch --features i18n
use mata_ui::{
    Window,
    i18n::I18n,
    widgets::{Button, Rect, WidgetEvent, WidgetWindow},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let translations = I18n::new("en")?;
    translations.add_resource("en", "save = Save\nswitch = Français")?;
    translations.add_resource("fr", "save = Enregistrer\nswitch = English")?;

    let win = WidgetWindow::from_window(Window::new("Mata i18n", (420, 150)))?;
    let save = Button::new(&win, Rect::new(16, 24, 180, 40), translations.tr("save"))?;
    let switch = Button::new(&win, Rect::new(214, 24, 180, 40), translations.tr("switch"))?;
    let mut events = Vec::new();
    loop {
        win.poll_events_into(&mut events);
        for event in &events {
            match event {
                WidgetEvent::Close => return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id == switch.id() => {
                    let target = if translations.locale().language() == "en" { "fr" } else { "en" };
                    translations.set_locale(target)?;
                    save.set_label(&translations.tr("save"));
                    switch.set_label(&translations.tr("switch"));
                }
                WidgetEvent::ButtonClicked(id) if *id == save.id() => {
                    println!("{}", translations.tr("save"));
                }
                _ => {}
            }
        }
        win.present();
    }
}
