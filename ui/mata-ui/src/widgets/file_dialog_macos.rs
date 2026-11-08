//! macOS file/folder chooser using the native Standard Additions dialogs via
//! `/usr/bin/osascript`. No Obj-C, GUI crate, or shell subprocess is required.
//! Dialogs return filesystem paths only; callers handle file I/O themselves.
use std::{io, path::PathBuf, process::Command};
use super::window::WidgetWindow;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum Kind { Open, Save, Folder }
#[derive(Debug,Clone)]
struct Filter { _name:String, extensions: Vec<String> }
pub struct FileDialog<'a> {
    _window: &'a WidgetWindow,
    kind: Kind,
    filters: Vec<Filter>,
    default_extension: Option<String>,
    title: Option<String>,
    initial: Option<PathBuf>,
    suggested_name: Option<String>,
}
fn dialog(window:&WidgetWindow,kind:Kind)->FileDialog<'_> {
    FileDialog { _window:window,kind,filters:Vec::new(),default_extension:None,
        title:None, initial:None,suggested_name:None }
}
pub fn open_files_dialog(window:&WidgetWindow)->FileDialog<'_> {dialog(window,Kind::Open)}
pub fn open_file_dialog(window:&WidgetWindow)->FileDialog<'_> {open_files_dialog(window)}
pub fn save_file_dialog(window:&WidgetWindow)->FileDialog<'_> {dialog(window,Kind::Save)}
pub fn open_folder_dialog(window:&WidgetWindow)->FileDialog<'_> {dialog(window,Kind::Folder)}
pub fn open_folders_dialog(window:&WidgetWindow)->FileDialog<'_> {open_folder_dialog(window)}
impl FileDialog<'_> {
    pub fn filter_extensions<I,S>(mut self, extensions:I)->Self
    where I: IntoIterator<Item=S>,S:AsRef<str> {
        self.filters.clear();
        self.filters.push(Filter {_name:"Supported files".into(),extensions:extensions.into_iter()
            .map(|x|x.as_ref().trim_start_matches('.').to_owned()).collect()});
        self
    }
    pub fn add_filter<I,S>(mut self,name:impl Into<String>,extensions:I)->Self
    where I: IntoIterator<Item=S>,S:AsRef<str> {
        self.filters.push(Filter {_name:name.into(),extensions:extensions.into_iter()
            .map(|x|x.as_ref().trim_start_matches('.').to_owned()).collect()});self
    }
    pub fn extension(mut self,value:impl AsRef<str>)->Self {
        self.default_extension=Some(value.as_ref().trim_start_matches('.').to_owned());self
    }
    pub fn title(mut self,value:impl Into<String>)->Self {self.title=Some(value.into());self}
    pub fn initial_path(mut self,value:impl Into<PathBuf>)->Self {self.initial=Some(value.into());self}
    pub fn initial_directory(self,value:impl Into<PathBuf>)->Self {self.initial_path(value)}
    pub fn suggested_name(mut self,value:impl Into<String>)->Self {self.suggested_name=Some(value.into());self}
    pub fn show(self)->io::Result<Option<PathBuf>> {Ok(self.run(false)?.and_then(|v|v.into_iter().next()))}
    pub fn show_files(self)->io::Result<Option<Vec<PathBuf>>> {
        if self.kind!=Kind::Open {return Err(io::Error::new(io::ErrorKind::InvalidInput,"show_files requires open file dialog"));}
        self.run(true)
    }
    pub fn show_folders(self)->io::Result<Option<Vec<PathBuf>>> {
        if self.kind!=Kind::Folder {return Err(io::Error::new(io::ErrorKind::InvalidInput,"show_folders requires folder dialog"));}
        self.run(true)
    }
    fn run(self,multiple:bool)->io::Result<Option<Vec<PathBuf>>> {
        let default_ext=self.default_extension.as_deref();
        for ext in self.filters.iter().flat_map(|f|&f.extensions).map(String::as_str)
            .chain(default_ext) {
            if !valid_extension(ext) {return Err(io::Error::new(io::ErrorKind::InvalidInput,
                format!("invalid extension: {ext:?}")));}
        }
        let title=self.title.as_deref().unwrap_or(match self.kind {
            Kind::Open=>"Open File",Kind::Save=>"Save File As",Kind::Folder=>"Choose Folder"});
        let title=script_string(title)?;
        let path=self.initial.as_deref().and_then(|path|{
            if path.is_dir() {Some(path)} else {path.parent().filter(|p|p.is_dir())}
        });
        let initial=match path {
            Some(path)=>format!(" default location (POSIX file {})",script_string(&path.to_string_lossy())?),
            None=>String::new(),
        };
        let script=match self.kind {
            Kind::Save=>{
                let default=self.suggested_name.as_deref().unwrap_or("Untitled");
                if default.contains('/') {return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid filename"));}
                format!("set chosen to choose file name with prompt {title} default name {}{initial}\nreturn POSIX path of chosen",
                    script_string(default)?)
            }
            Kind::Open | Kind::Folder=>{
                let mut command=format!("choose {} with prompt {title}",
                    if self.kind==Kind::Open {"file"} else {"folder"});
                if self.kind==Kind::Open {
                    let mut exts=Vec::<String>::new();
                    for ext in self.filters.iter().flat_map(|f|&f.extensions) {
                        if !exts.iter().any(|e|e==ext){exts.push(ext.clone());}
                    }
                    if !exts.is_empty() {
                        command.push_str(" of type {");
                        command.push_str(&exts.iter().map(|e|script_string(e)).collect::<io::Result<Vec<_>>>()?.join(", "));
                        command.push('}');
                    }
                }
                command.push_str(&initial);
                if multiple {command.push_str(" with multiple selections allowed");}
                if multiple {
                    format!("set chosen to {command}\nset paths to \"\"\nrepeat with itemPath in chosen\nset paths to paths & (POSIX path of itemPath) & (ASCII character 10)\nend repeat\nreturn paths")
                } else {format!("set chosen to {command}\nreturn POSIX path of chosen")}
            }
        };
        let output=Command::new("/usr/bin/osascript").args(["-e",&script]).output()?;
        if !output.status.success() {
            let message=String::from_utf8_lossy(&output.stderr);
            if message.contains("(-128)") || message.contains("User canceled") || message.contains("User cancelled") {
                return Ok(None);
            }
            return Err(io::Error::other(format!("macOS file chooser failed: {message}")));
        }
        let result=String::from_utf8(output.stdout).map_err(|e|io::Error::new(io::ErrorKind::InvalidData,e))?;
        let mut paths=result.trim_end_matches(['\r','\n']).split('\n')
            .filter(|s|!s.is_empty()).map(PathBuf::from).collect::<Vec<_>>();
        if paths.is_empty(){return Ok(None);}
        if self.kind==Kind::Save && paths[0].extension().is_none() {
            if let Some(ext)=default_ext{paths[0].set_extension(ext);}
        }
        Ok(Some(paths))
    }
}
fn valid_extension(s:&str)->bool {
    !s.is_empty() && s.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'||c=='-')
}
fn script_string(input:&str)->io::Result<String>{
    if input.chars().any(|c|matches!(c,'\0'|'\r'|'\n')) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid control character in dialog argument"));
    }
    Ok(format!("\"{}\"",input.replace('\\',"\\\\").replace('"',"\\\"")))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn apple_script_strings_escape_quotes() {
        assert_eq!(script_string("name \"file\"").unwrap(), "\"name \\\"file\\\"\"");
        assert!(script_string("bad\nname").is_err());
    }
}
