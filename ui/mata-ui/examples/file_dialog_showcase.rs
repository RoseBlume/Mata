//! Run: cargo run -p mata-ui --example file_dialog_showcase
//! File chooser examples for Linux portals, Windows, and macOS.
#[path = "roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window = support::host("Mata file & folder dialogs")?;
    let open = Button::new(&window, Rect::new(20, 65, 310, 48), "Open file")?;
    let multi = Button::new(&window, Rect::new(350, 65, 310, 48), "Open multiple files")?;
    let save = Button::new(&window, Rect::new(20, 137, 310, 48), "Save As")?;
    let folder = Button::new(&window, Rect::new(350, 137, 310, 48), "Choose folder")?;
    let folders = Button::new(&window, Rect::new(20, 209, 310, 48), "Choose multiple folders")?;
    let filtered = Button::new(&window, Rect::new(350, 209, 310, 48), "Named file filters")?;
    window.add_auto_content(20, 12, Some(630), None,
        "Try each picker. Cancel returns None; chosen paths print to the terminal.");
    window.add_content(Rect::new(20, 286, 630, 65),
        "Linux: XDG Desktop Portal first; helpers as fallback. Windows/macOS: native dialogs.");
    support::drive(window, |host, event| {
        let WidgetEvent::ButtonClicked(id) = event else {return;};
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        {
            if *id == open.id() {
                println!("Open: {:?}", open_file_dialog(host).title("Open a file").show());
            } else if *id == multi.id() {
                println!("Multiple files: {:?}", open_files_dialog(host).show_files());
            } else if *id == save.id() {
                println!("Save: {:?}", save_file_dialog(host).title("Save a document")
                    .extension("txt").suggested_name("example.txt").show());
            } else if *id == folder.id() {
                println!("Folder: {:?}", open_folder_dialog(host).show());
            } else if *id == folders.id() {
                println!("Multiple folders: {:?}", open_folders_dialog(host).show_folders());
            } else if *id == filtered.id() {
                println!("Filtered: {:?}", open_file_dialog(host)
                    .add_filter("Images", ["png", "jpg", "jpeg", "webp"])
                    .add_filter("Documents", ["txt", "md", "pdf"])
                    .show());
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let _=(id,host,&open,&multi,&save,&folder,&folders,&filtered);
    });
    Ok(())
}
