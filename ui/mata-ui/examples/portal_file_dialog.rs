//! Test under COSMIC, GNOME, KDE Plasma, XFCE, LXQt, or another Linux desktop.
//! cargo run -p mata-ui --example portal_file_dialog
#[path = "roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window = support::host("Mata XDG FileChooser portal")?;
    window.add_auto_content(20, 12, Some(630), None,
        "Auto uses the desktop portal; Portal forces D-Bus (no helper fallback).");
    let auto = Button::new(&window, Rect::new(20, 70, 300, 48), "Auto: choose file")?;
    let portal = Button::new(&window, Rect::new(340, 70, 300, 48), "Portal only: choose file")?;
    let multi = Button::new(&window, Rect::new(20, 135, 300, 48), "Portal: multiple files")?;
    let folder = Button::new(&window, Rect::new(340, 135, 300, 48), "Portal: folder")?;
    let save = Button::new(&window, Rect::new(20, 200, 300, 48), "Portal: save as")?;
    let folders = Button::new(&window, Rect::new(340, 200, 300, 48), "Portal: multiple folders")?;
    support::drive(window, |host, event| {
        let WidgetEvent::ButtonClicked(id) = event else { return; };
        if *id == auto.id() {
            println!("Auto: {:?}", open_file_dialog(host).show());
        } else if *id == portal.id() {
            println!("Portal: {:?}", open_file_dialog(host).backend(DialogBackend::Portal).show());
        } else if *id == multi.id() {
            println!("Multiple: {:?}", open_files_dialog(host).backend(DialogBackend::Portal)
                .add_filter("Images", ["png", "jpeg", "webp"])
                .add_filter("Text", ["md", "txt"]).show_files());
        } else if *id == folder.id() {
            println!("Folder: {:?}", open_folder_dialog(host).backend(DialogBackend::Portal).show());
        } else if *id == save.id() {
            println!("Save: {:?}", save_file_dialog(host).backend(DialogBackend::Portal)
                .title("Save a document").suggested_name("notes.txt")
                .extension("txt").show());
        } else if *id == folders.id() {
            println!("Folders: {:?}", open_folders_dialog(host).backend(DialogBackend::Portal)
                .show_folders());
        }
    });
    Ok(())
}
