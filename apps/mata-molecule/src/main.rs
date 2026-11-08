mod draw;
mod lod;
mod render_budget;
mod gpu_shaders;
#[cfg(target_os="windows")] mod d3d11_studio;
mod electron;
mod orbitals;
mod window_fullscreen;
mod view;
mod ribbon;
mod font;
mod file_dialog;
mod protein_structure;
mod camera_motion;
mod cell_view;
mod virus_view;
use mata_molecule::cell::{CellData,GeneratedCell,load_volume};
use mata_molecule::virus::{VirusModel,ViralComponent};
use virus_view::VirusViewer;
use render_budget::RenderPreset;
use std::collections::HashMap;
use cell_view::{CellViewer,VolumeMode};
use mata_molecule::protein_inspect::{self,Residue};
use draw::{Rect,BG,PANEL,PANEL2,STROKE,MUTED,WHITE,BLUE,GOLD};
use mata_win::{Window,Framebuffer,events::{WindowEvent,Key}};
use mata_molecule::{Molecule,Vec3,propose,read_document,OpenedDocument,ProteinRecord,write_structures,chemistry::layout3d,vsepr_centers};
use std::time::{Duration, Instant};
use camera_motion::CameraMotion;
use mata_molecule::reconstruction::{self, ReconstructionSettings, Reconstructed};
use std::sync::mpsc::{self,Receiver};

#[derive(Clone,Copy,PartialEq,Eq,Debug)]enum Tab{Lewis,Model3d,Vsepr,Record,Files,Residues,Amino,Settings,Cell,Virus}
impl Tab {
    const ALL:[Self;10]=[Self::Lewis,Self::Model3d,Self::Cell,Self::Virus,
        Self::Vsepr,Self::Residues,Self::Amino,Self::Record,Self::Files,Self::Settings];
    fn label(self)->&'static str {match self {
        Self::Lewis=>"Lewis structures",Self::Model3d=>"3D molecular viewer",
        Self::Cell=>"Cell explorer",Self::Virus=>"Virus explorer",
        Self::Vsepr=>"VSEPR geometry",Self::Residues=>"Protein residues",
        Self::Amino=>"Amino acid detail",Self::Record=>"Protein records",
        Self::Files=>"Loaded files",Self::Settings=>"Settings",
    }}
    fn nav_name(self)->&'static str {match self {
        Self::Lewis=>"Lewis diagrams",Self::Model3d=>"3D molecules",
        Self::Cell=>"Cells",Self::Virus=>"Viruses",
        Self::Vsepr=>"VSEPR shapes",Self::Residues=>"Residues",
        Self::Amino=>"Amino acids",Self::Record=>"Protein record",
        Self::Files=>"Files",Self::Settings=>"Settings",
    }}
    fn short(self)->&'static str {match self {
        Self::Lewis=>"2D",Self::Model3d=>"3D",Self::Cell=>"CL",Self::Virus=>"VI",
        Self::Vsepr=>"AX",Self::Residues=>"RS",Self::Amino=>"AA",Self::Record=>"PR",
        Self::Files=>"FI",Self::Settings=>"ST",
    }}
    fn description(self)->&'static str {match self {
        Self::Lewis=>"Build or import a molecule and inspect electron pairs and bonds",
        Self::Model3d=>"Rotate, explore protein chains and switch display modes",
        Self::Cell=>"Explore simulated organelles or measured microscopy volumes",
        Self::Virus=>"Inspect viral models, density maps and linked proteins",
        Self::Vsepr=>"Geometry and bond angles for each central atom",
        Self::Residues=>"Browse chains, residues and backbone angles",
        Self::Amino=>"Inspect the chemistry of the selected amino acid",
        Self::Record=>"Sequence, source records and predicted 3D links",
        Self::Files=>"Manage multiple imported structures and visibility",
        Self::Settings=>"Reconstruction controls and display preferences",
    }}
    fn has_variant(self)->bool{matches!(self,Self::Lewis|Self::Model3d|Self::Vsepr|Self::Record|Self::Residues|Self::Amino)}
    fn is_view(self)->bool{matches!(self,Self::Lewis|Self::Model3d|Self::Cell|Self::Virus)}
}

/// Geometry for the new navigation-first interface. Rendering and hit-testing
/// both use these rectangles to avoid toolbar/header click mismatches.
#[derive(Clone,Copy)]struct UiShell {width:i32,height:i32,rail:i32,content:Rect,immersive:bool}
impl UiShell {
    fn new(width:i32,height:i32,tab:Tab)->Self {
        let rail=if width>=960 {196}else{66};
        let top=match tab {Tab::Lewis=>215,Tab::Model3d|Tab::Vsepr|Tab::Record|Tab::Residues|Tab::Amino=>163,_=>99};
        let x=rail+18;
        let content=Rect::new(x,top,(width-x-18).max(120),(height-top-83).max(115));
        Self{width,height,rail,content,immersive:false}
    }
    fn immersive(width:i32,height:i32)->Self {
        Self{width,height,rail:0,content:Rect::new(8,8,(width-16).max(1),(height-16).max(1)),immersive:true}
    }
    fn nav_rect(self,tab:Tab)->Rect {
        // Navigation groups: Explore, Analyze, Workspace
        let y=match tab {
            Tab::Lewis=>120,Tab::Model3d=>159,Tab::Cell=>198,Tab::Virus=>237,
            Tab::Vsepr=>315,Tab::Residues=>354,Tab::Amino=>393,Tab::Record=>432,
            Tab::Files=>507,Tab::Settings=>546,
        };
        Rect::new(8,y,self.rail-16,35)
    }
    fn open_rect(self)->Rect{Rect::new(self.width-248,19,106,39)}
    fn save_rect(self)->Rect{Rect::new(self.width-131,19,111,39)}
    fn details_rect(self)->Rect{Rect::new(self.width-365,19,106,39)}
    fn formula_rect(self)->Rect{Rect::new(self.rail+29,119,(self.width-self.rail-204).max(130),38)}
    fn build_rect(self)->Rect{Rect::new(self.width-162,119,133,38)}
    fn variant_rect(self,tab:Tab)->Rect {
        let y=if tab==Tab::Lewis {187}else{129};
        Rect::new(self.rail+30,y,(self.width-self.rail-210).max(70),12)
    }
    fn path_rect(self)->Rect{Rect::new(self.rail+90,self.height-40,(self.width-self.rail-120).max(150),31)}
    fn help_rect(self)->Rect{Rect::new(8,self.height-130,self.rail-16,35)}
    fn inspector_rect(self,visible:bool)->Rect {
        let area=self.content;
        if !visible || (!self.immersive && area.w<390) {return Rect::new(self.width+24,area.y,250,area.h);}
        let w=if self.immersive {(area.w/4).clamp(130,365).min((area.w-120).max(1))}
            else if area.w>=750 {area.w.min(1200)/4}
            else {(area.w/3).clamp(160,220)};
        Rect::new(area.x+area.w-w,area.y,w,area.h)
    }
    fn canvas_rect(self,inspector:bool)->Rect {
        let area=self.content;
        let side=self.inspector_rect(inspector);
        if side.x>self.width {area} else {Rect::new(area.x,area.y,area.w-side.w-12,area.h)}
    }
}

#[derive(Clone,Copy,PartialEq,Eq)]enum Focus{Formula,File,Neither}
struct LoadedFile {
    path: String,
    models: Vec<Molecule>,
    index: usize,
    visible: bool,
    record: Option<ProteinRecord>,
    residues: Vec<Residue>,
    selected_residue: Option<usize>,
    preview: Option<Reconstructed>,
}
struct Studio {
    window:Window,fb:Framebuffer,events:Vec<WindowEvent>,models:Vec<Molecule>,index:usize,
    formula:String,path:String,file_input:String,tab:Tab,focus:Focus,status:String,viewer:view::Molecule3d,
    mouse:(i32,i32),drag_rotate:bool,drag_pan:bool,drag_variant:bool,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),camera_motion:CameraMotion,detail_mode:view::DetailMode,focused_chain:Option<String>,residue_colors:bool,vsepr_scroll:usize,
    font_info:String,record:Option<ProteinRecord>,
    active_visible:bool,other_files:Vec<LoadedFile>,initial_draft:bool,
    residues:Vec<Residue>,selected_residue:Option<usize>,residue_scroll:usize,focus_residue:bool,file_scroll:usize,
    pending_model:Option<(String,Receiver<Result<(Vec<Molecule>,String),String>>)>,
    settings: ReconstructionSettings, preview:Option<Reconstructed>,
    cell_data:Vec<CellData>,cell_index:usize,cell_selected:Option<usize>,cell_scroll:usize,cell_slice:f32,cell_gain:f32,cell_threshold:f32,cell_mode:VolumeMode,cell_viewer:CellViewer,drag_cell_slice:bool,cell_probe:Option<([usize;3],f32)>,
    virus:VirusModel,virus_viewer:VirusViewer,virus_experimental:bool,virus_density:Option<usize>,virus_mode:VolumeMode,virus_threshold:f32,drag_virus_contour:bool,
    virus_selected:Option<ViralComponent>,virus_links:HashMap<ViralComponent,String>,
    inspector_open:bool,help_open:bool,
    viewer_expanded:bool,viewer_focused:bool,app_fullscreen:window_fullscreen::FullscreenState,
    f11_held:bool,f_held:bool,
    selected_atom:Option<usize>,show_electrons:bool,orbital_mask:u8,pick_origin:Option<(i32,i32)>,electron_neighbor_scroll:usize,
    /// GPU failure disables only the optional 3D compositor, never the editor.
    #[cfg(target_os="windows")]
    d3d11:Option<d3d11_studio::Layer>,
    gpu_enabled:bool,
    gpu_label:String,
    render_preset:RenderPreset,
}
fn hit_key(k:Key,shift:bool)->Option<char>{match k{
    Key::Char(c)=>Some(if shift {match c{
        '='=>'+','-'=>'_',';'=>':',','=>'<','.'=>'>','/'=>'?',
        '['=>'{',']'=>'}','\\'=>'|','\''=>'"','`'=>'~',
        _=>c.to_ascii_uppercase(),
    }}else{c}),
    Key::Num(n) if n<=9=>Some(if shift{match n{
        0=>')',1=>'!',2=>'@',3=>'#',4=>'$',5=>'%',6=>'^',7=>'&',8=>'*',9=>'(',_=>unreachable!(),
    }}else{char::from(b'0'+n)}),
    Key::Space=>Some(' '),_=>None,
}}
/// Layout is shared between rendering and mouse hit-testing. The two groups
/// wrap independently to keep the toolbar usable in narrower windows.
/// 0 backbone, 1 atoms, 2 ribbon, 3 reset, 4 residue focus,
/// 5 next chain, 6 all chains, 7 amino-acid colors, 8 active visibility.
fn model_toolbar_layout(area:Rect)->[Rect;9] {
    let left=area.x+12;
    let right=area.x+area.w-12;
    let available=(right-left).max(1);
    let mut rects=[Rect::new(0,0,0,0);9];
    let widths=[115,115,115,112,146,125,128,148,144];
    let mut x=left;
    let mut y=area.y+12;
    for (i,&desired) in widths.iter().enumerate() {
        if i==5 {x=left;y+=40;}
        let width=desired.min(available);
        if x!=left && x+width>right {x=left;y+=40;}
        rects[i]=Rect::new(x,y,width,32);
        x+=width+8;
    }
    rects
}

impl Studio {
    fn new()->Self{
        let settings=ReconstructionSettings::load();
        let (w,h)=(1180,760);
        let models=propose("C2H6O").unwrap_or_default();
        let preview=models.first().map(|m|reconstruction::reconstruct(m,settings));
        let window=Window::new("Mata Molecule Studio",(w,h));
        #[cfg(target_os="windows")]
        let (d3d11,gpu_label)=match d3d11_studio::Layer::new(window.handle(),w,h) {
            Ok(layer)=>{let name=layer.name().to_owned();
                eprintln!("[Mata Molecule] D3D11 enabled: {name}");
                (Some(layer),format!("D3D11: {name}"))},
            Err(error)=>{eprintln!("[Mata Molecule] software rendering: {error}");
                (None,format!("CPU ({error})"))},
        };
        #[cfg(not(target_os="windows"))]
        let gpu_label="CPU (D3D11 supported on Windows only)".to_owned();
        #[cfg(target_os="windows")]
        let gpu_enabled=d3d11.is_some();
        #[cfg(not(target_os="windows"))]
        let gpu_enabled=false;
        Self{window,fb:Framebuffer::from_color(w,h,BG),events:Vec::new(),models,index:0,
        formula:"C2H6O".into(),path:"molecule.sdf".into(),file_input:"molecule.sdf".into(),tab:Tab::Lewis,focus:Focus::Formula,
        status:"READY | Formula inference gives candidates, not verified molecular identities".into(),viewer:view::Molecule3d::new(),
        mouse:(0,0),drag_rotate:false,drag_pan:false,drag_variant:false,ax:-0.35,ay:0.48,zoom:1.0,pan:(0.0,0.0),camera_motion:CameraMotion::default(),detail_mode:view::DetailMode::Backbone,focused_chain:None,residue_colors:true,vsepr_scroll:0,
        font_info:"Not initialized".into(),record:None,pending_model:None,settings,preview,
        active_visible:true,other_files:Vec::new(),initial_draft:true,residues:Vec::new(),selected_residue:None,
        residue_scroll:0,focus_residue:false,file_scroll:0,cell_data:vec![CellData::Generated(GeneratedCell::animal()),CellData::Generated(GeneratedCell::plant())],
        cell_index:0,cell_selected:Some(1),cell_scroll:0,cell_slice:0.5,cell_gain:1.0,cell_threshold:0.5,cell_mode:VolumeMode::MaxIntensity,cell_viewer:CellViewer::new(),drag_cell_slice:false,cell_probe:None,
        virus:VirusModel::sars_cov_2(),virus_viewer:VirusViewer::new(),virus_experimental:false,
        virus_density:None,virus_mode:VolumeMode::Surface,virus_threshold:0.5,drag_virus_contour:false,virus_selected:Some(ViralComponent::Spike),virus_links:HashMap::new(),inspector_open:true,help_open:false,
        viewer_expanded:false,viewer_focused:false,app_fullscreen:window_fullscreen::FullscreenState::new(),
        selected_atom:None,show_electrons:false,orbital_mask:15,pick_origin:None,electron_neighbor_scroll:0,
            #[cfg(target_os="windows")] d3d11, gpu_enabled,gpu_label,
        f11_held:false,f_held:false,render_preset:RenderPreset::Auto}
    }
    fn w(&self)->i32{self.fb.width as i32}
    fn h(&self)->i32{self.fb.height as i32}
    fn ui(&self)->UiShell{
        if self.viewer_expanded && self.has_3d_viewer(){UiShell::immersive(self.w(),self.h())}
        else{UiShell::new(self.w(),self.h(),self.tab)}
    }
    fn has_3d_viewer(&self)->bool{matches!(self.tab,Tab::Model3d|Tab::Cell|Tab::Virus)}
    fn toggle_viewer_expanded(&mut self) {
        if !self.has_3d_viewer(){return;}
        self.viewer_expanded=!self.viewer_expanded;
        self.viewer_focused=true;
        self.focus=Focus::Neither;
        self.drag_rotate=false;self.drag_pan=false;
        self.status=if self.viewer_expanded{"FULL VIEW | F OR ESC TO EXIT | DETAILS DOCKED RIGHT".into()}
            else{"NORMAL WORKSPACE | F TO ENTER FULL VIEW".into()};
    }
    fn viewer_toggle_rect(&self)->Rect {
        let c=self.canvas_rect();
        Rect::new(c.x+c.w-144,c.y+7,134,30)
    }
    fn draw_viewer_toggle(&mut self) {
        if self.has_3d_viewer(){
            let button=self.viewer_toggle_rect();
            self.button(button,if self.viewer_expanded{"EXIT FULL VIEW"}else{"FULL VIEW [F]"},self.viewer_expanded);
        }
    }
    fn toggle_app_fullscreen(&mut self) {
        self.status=match self.app_fullscreen.toggle(self.window.handle()) {
            Ok(true)=>"APP FULLSCREEN ENABLED | F11 TO RESTORE".into(),
            Ok(false)=>"APP WINDOW RESTORED".into(),
            Err(e)=>format!("FULLSCREEN ERROR: {e}"),
        };
    }
    fn formula_rect(&self)->Rect{self.ui().formula_rect()}
    fn file_rect(&self)->Rect{self.ui().path_rect()}
    fn generate_rect(&self)->Rect{self.ui().build_rect()}
    fn open_rect(&self)->Rect{self.ui().open_rect()}
    fn save_rect(&self)->Rect{self.ui().save_rect()}
    fn slider_rect(&self)->Rect{self.ui().variant_rect(self.tab)}
    fn tab_rect(&self,t:Tab)->Rect{self.ui().nav_rect(t)}
    fn canvas_rect(&self)->Rect{self.ui().canvas_rect(self.viewer_expanded||self.inspector_open)}
    fn sidebar_rect(&self)->Rect{self.ui().inspector_rect(self.viewer_expanded||self.inspector_open)}
    fn report_rect(&self)->Rect{self.ui().content}
    fn set_tab(&mut self,tab:Tab) {
        let previous=self.tab;
        if previous==tab {return;}
        if tab==Tab::Virus && previous==Tab::Cell && matches!(self.cell_selected_data(),Some(CellData::Microscopy(_))) {
            self.virus_density=Some(self.cell_index);
        }
        self.tab=tab;
        self.viewer_expanded=false;
        self.viewer_focused=false;
        self.focus=Focus::Neither;
        self.drag_rotate=false;self.drag_pan=false;self.drag_variant=false;
        self.pick_origin=None;
        self.drag_cell_slice=false;self.drag_virus_contour=false;
        self.camera_motion.reset();
        if matches!(tab,Tab::Cell|Tab::Virus) && !matches!(previous,Tab::Cell|Tab::Virus) {self.reset_camera();}
    }
    fn current(&self)->Option<&Molecule>{self.models.get(self.index)}
    fn display_current(&self)->Option<&Molecule>{
        self.preview.as_ref().map(|p|&p.molecule).or_else(||self.current())
    }
    fn refresh_preview(&mut self){
        self.preview=self.current().map(|m|reconstruction::reconstruct(m,self.settings));
        self.viewer.invalidate_scene();
    }
    fn refresh_all_previews(&mut self){
        self.refresh_preview();
        for file in &mut self.other_files {
            file.preview=file.models.get(file.index).map(|m|reconstruction::reconstruct(m,self.settings));
        }
    }
    fn set_setting(&mut self,idx:usize){
        match idx {
            0=>self.settings.hydrogens=!self.settings.hydrogens,
            1=>self.settings.bonds=!self.settings.bonds,
            2=>self.settings.side_chains=!self.settings.side_chains,
            3=>self.settings.backbone=!self.settings.backbone,
            4=>self.settings.highlight=!self.settings.highlight,
            _=>return,
        }
        self.refresh_all_previews();
        self.settings.save();
        self.status="RECONSTRUCTION SETTINGS UPDATED | ORIGINAL FILES AND COORDINATES UNCHANGED".into();
    }
    fn setting_row(&self,idx:usize)->Rect{
        let r=self.report_rect();Rect::new(r.x+22,r.y+55+idx as i32*46,r.w-44,39)
    }
    fn take_active(&mut self)->LoadedFile {
        LoadedFile{path:std::mem::take(&mut self.path),models:std::mem::take(&mut self.models),
            index:std::mem::replace(&mut self.index,0),
            visible:std::mem::replace(&mut self.active_visible,true),
            record:self.record.take(),residues:std::mem::take(&mut self.residues),
            selected_residue:self.selected_residue.take(),preview:self.preview.take()}
    }
    fn use_loaded(&mut self, file:LoadedFile) {
        self.path=file.path;self.file_input=self.path.clone();self.models=file.models;self.index=file.index;
        self.active_visible=file.visible;self.record=file.record;self.preview=file.preview;
        if self.preview.is_none(){self.refresh_preview();}
        self.residues=file.residues;self.selected_residue=file.selected_residue;
        self.formula=self.current().map_or(String::new(),Molecule::formula);
        self.residue_scroll=0;self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;self.focused_chain=None;self.reset_camera();self.pending_model=None;self.vsepr_scroll=0;
    }
    fn select_file(&mut self,row:usize){
        if row==0{return;}
        let i=row-1;
        if i>=self.other_files.len(){return;}
        let previous=self.take_active();
        let selected=std::mem::replace(&mut self.other_files[i],previous);
        self.use_loaded(selected);
        self.status=format!("ACTIVE FILE: {}",self.path);
    }
    fn remove_file(&mut self,row:usize){
        if row==0 {
            if self.other_files.is_empty(){
                self.models.clear();self.preview=None;self.record=None;self.residues.clear();self.selected_residue=None;
                self.active_visible=false;self.status="UNLOADED LAST FILE".into();
            }else{
                let selected=self.other_files.remove(0);self.use_loaded(selected);
            }
        }else if row-1<self.other_files.len(){self.other_files.remove(row-1);}
    }
    fn toggle_file(&mut self,row:usize){
        if row==0 {self.active_visible=!self.active_visible;}
        else if let Some(f)=self.other_files.get_mut(row-1){f.visible=!f.visible;}
    }
    fn cache_residues(&mut self){
        self.residues=self.current().map(protein_inspect::residues).unwrap_or_default();
        self.selected_residue=if self.residues.is_empty(){None}else{Some(0)};
        self.residue_scroll=0;self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;
        self.refresh_preview();
    }
    fn add_file(&mut self, mut file:LoadedFile){
        if file.models.is_empty() && file.record.is_none(){return;}
        if self.initial_draft {
            self.models.clear();self.record=None;self.initial_draft=false;
        } else if !self.models.is_empty() || self.record.is_some(){
            let prior=self.take_active();
            self.other_files.push(prior);
        }
        file.index=file.index.min(file.models.len().saturating_sub(1));
        self.use_loaded(file);
    }
    fn file_count(&self)->usize {
        self.other_files.len()+usize::from(!self.models.is_empty() || self.record.is_some())
    }
    fn model_buttons(&self)->[Rect;9] {
        let c=self.canvas_rect();
        model_toolbar_layout(Rect::new(c.x,c.y+36,c.w,(c.h-36).max(1)))
    }
    fn model_toolbar_bottom(&self)->i32 {
        self.model_buttons().iter().map(|r|r.y+r.h).max().unwrap_or(self.canvas_rect().y)
    }
    fn toggle_bonds(&mut self) {
        self.detail_mode=if self.detail_mode==view::DetailMode::AllAtoms {
            view::DetailMode::Backbone
        } else {view::DetailMode::AllAtoms};
        self.status=if self.detail_mode==view::DetailMode::AllAtoms {
            "ALL ATOMS: ELEMENT-COLORED ATOMS AND ALL AVAILABLE BONDS".into()
        }else{"C-ALPHA BACKBONE VIEW".into()};
    }
    fn toggle_ribbon(&mut self) {
        if self.detail_mode==view::DetailMode::Ribbon {
            self.detail_mode=view::DetailMode::Backbone;
            self.status="C-ALPHA BACKBONE VIEW".into();
        } else if self.current().is_some_and(view::supports_ribbon) {
            self.detail_mode=view::DetailMode::Ribbon;
            self.status="PROTEIN RIBBON: HELICES, EXTENDED STRANDS AND LOOPS ESTIMATED FROM BACKBONE GEOMETRY".into();
        } else {
            self.status="RIBBON MODE REQUIRES OBSERVED C-ALPHA BACKBONE COORDINATES".into();
        }
    }
    fn set_all_files_visible(&mut self,on:bool) {
        self.active_visible=on && (!self.models.is_empty() || self.record.is_some());
        for file in &mut self.other_files {file.visible=on;}
        self.status=if on{"ALL LOADED FILES: VISIBLE".into()}else{"ALL LOADED FILES: HIDDEN (USE SHOW ALL OR FILES TAB TO RESTORE)".into()};
    }
    fn clear_chain(&mut self) {
        if self.focused_chain.take().is_some() {
            self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;
            self.reset_camera();
        }
        self.status="SHOWING ALL PROTEIN CHAINS".into();
    }
    fn cycle_chain(&mut self) {
        let chains=self.chains();
        if chains.is_empty() {self.status="NO PROTEIN CHAINS IN ACTIVE STRUCTURE".into();return;}
        let index=self.focused_chain.as_ref().and_then(|value|chains.iter().position(|c|c==value));
        if let Some(chain)=index.and_then(|idx|chains.get(idx+1)).or_else(||if index.is_none(){chains.first()}else{None}) {
            self.select_chain(chain.clone());
        }else{self.clear_chain();}
    }
    fn click_model_control(&mut self,index:usize) {
        match index {
            0=>{self.detail_mode=view::DetailMode::Backbone;self.status="C-ALPHA BACKBONE VIEW".into();}
            1=>self.toggle_bonds(),
            2=>self.toggle_ribbon(),
            3=>{self.reset_camera();self.selected_atom=None;self.show_electrons=false;},
            4=>if let Some(i)=self.selected_residue {
                if self.focus_residue {
                    self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;
                    self.status="RESIDUE FOCUS DISABLED".into();
                }else{self.focus_on_residue(i);}
            }else{self.status="SELECT AN AMINO ACID IN THE RESIDUES TAB FIRST".into();},
            5=>self.cycle_chain(),
            6=>self.clear_chain(),
            7=>{
                self.residue_colors=!self.residue_colors;
                self.status=if self.residue_colors{"AMINO-ACID CLASS COLORS ENABLED FOR ISOLATED CHAINS".into()}
                    else{"AMINO-ACID CLASS COLORS DISABLED".into()};
            },
            8=>{
                self.active_visible=!self.active_visible;
                self.status=if self.active_visible{"ACTIVE STRUCTURE VISIBLE".into()}else{"ACTIVE STRUCTURE HIDDEN".into()};
            },
            _=>{},
        }
    }
    fn draw_model_toolbar(&mut self) {
        let controls=self.model_buttons();
        let labels=["BACKBONE","ALL ATOMS","RIBBON","RESET VIEW",
            if self.focus_residue {"EXIT FOCUS"}else{"FOCUS RESIDUE"},
            "NEXT CHAIN","ALL CHAINS",
            if self.residue_colors {"AA COLORS: ON"}else{"AA COLORS: OFF"},
            if self.active_visible {"HIDE ACTIVE"}else{"SHOW ACTIVE"}];
        let selected=[self.detail_mode==view::DetailMode::Backbone,
            self.detail_mode==view::DetailMode::AllAtoms,
            self.detail_mode==view::DetailMode::Ribbon,
            false,self.focus_residue,self.focused_chain.is_some(),self.focused_chain.is_none(),
            self.residue_colors,self.active_visible];
        for (i,rect) in controls.iter().copied().enumerate(){
            self.button(rect,labels[i],selected[i]);
        }
    }
    fn files_show_all_button(&self)->Rect {
        let r=self.report_rect();Rect::new(r.x+r.w-508,r.y+15,145,35)
    }
    fn files_hide_all_button(&self)->Rect {
        let r=self.report_rect();Rect::new(r.x+r.w-352,r.y+15,145,35)
    }
    fn settings_defaults_button(&self)->Rect {
        let r=self.report_rect();Rect::new(r.x+r.w-183,r.y+11,160,34)
    }
    fn chains(&self)->Vec<String> {
        let mut out=std::collections::BTreeSet::new();
        if let Some(m)=self.current(){for a in &m.atoms {
            if let Some(site)=&a.site {
                if site.atom_name.trim()=="CA" && !site.hetero {
                    out.insert(site.chain_id.clone());
                }
            }
        }}
        out.into_iter().collect()
    }
    fn cell_buttons(&self)->[Rect;11] {
        let c=self.canvas_rect();let available=c.w-24;
        let widths=[108,108,112,110,97,97,97,97,92,92,110];
        let mut out=[Rect::new(0,0,0,0);11];
        let (mut x,mut y)=(c.x+12,c.y+47);
        for (i,ww) in widths.into_iter().enumerate() {
            let w=ww.min(available.max(1));
            if x>c.x+12 && x+w>c.x+c.w-12 {x=c.x+12;y+=36;}
            out[i]=Rect::new(x,y,w,29);x+=w+6;
        }
        out
    }
    fn cell_slider(&self)->Rect {
        let c=self.canvas_rect();let end=self.cell_buttons().iter().map(|r|r.y+r.h).max().unwrap_or(c.y);
        Rect::new(c.x+22,end+23,(c.w-44).max(20),9)
    }
    fn cell_image_rect(&self)->Rect {
        let c=self.canvas_rect();let bottom=self.cell_slider().y+27;
        Rect::new(c.x+2,bottom,c.w-4,(c.y+c.h-bottom-2).max(1))
    }
    fn virus_buttons(&self)->[Rect;10] {
        let c=self.canvas_rect();let widths=[120,132,96,122,120,116,115,98,118,100];
        let mut result=[Rect::new(0,0,0,0);10];
        let (mut x,mut y)=(c.x+12,c.y+47);
        for (i,w) in widths.into_iter().enumerate(){
            let w=w.min((c.w-24).max(1));
            if x>c.x+12&&x+w>c.x+c.w-12{x=c.x+12;y+=35;}
            result[i]=Rect::new(x,y,w,29);x+=w+6;
        }
        result
    }
    fn virus_contour_slider(&self)->Rect {
        let c=self.canvas_rect();
        let end=self.virus_buttons().iter().map(|b|b.y+b.h).max().unwrap_or(c.y);
        Rect::new(c.x+22,end+23,(c.w-44).max(20),9)
    }
    fn virus_image_rect(&self)->Rect {
        let c=self.canvas_rect();
        let bottom=self.virus_contour_slider().y+26;
        Rect::new(c.x+2,bottom,c.w-4,(c.y+c.h-bottom-2).max(1))
    }
    fn attach_active_protein(&mut self) {
        let Some(kind)=self.virus_selected else {self.status="SELECT A VIRAL PROTEIN IN THE SIDEBAR FIRST".into();return;};
        if !kind.accepts_protein() {self.status="THIS VIRION COMPONENT IS NOT A PROTEIN".into();return;}
        if self.models.get(self.index).is_none_or(|m| !m.atoms.iter().any(|a|a.site.is_some())) {
            self.status="LOAD A PDB/MMCIF PROTEIN FILE, THEN CLICK LINK ACTIVE".into();return;
        }
        self.virus_links.insert(kind,self.path.clone());
        self.status=format!("LINKED {} TO {} | SOURCE COORDINATES UNCHANGED",kind.name(),self.path);
    }
    fn inspect_virus_protein(&mut self) {
        let Some(kind)=self.virus_selected else {self.status="SELECT A VIRAL PROTEIN FIRST".into();return;};
        let Some(path)=self.virus_links.get(&kind).cloned() else {
            self.status=format!("NO STRUCTURE LINKED TO {} | OPEN A PDB/MMCIF AND LINK IT",kind.name());return;
        };
        if self.path!=path {
            let Some(i)=self.other_files.iter().position(|f|f.path==path) else {
                self.status="LINKED PROTEIN WAS REMOVED; IMPORT IT AGAIN".into();return;
            };
            self.select_file(i+1);
        }
        self.tab=Tab::Model3d;
        self.detail_mode=view::DetailMode::Ribbon;
        self.active_visible=true;
        self.status=format!("INSPECTING {} | {} | SELECT RESIDUES OR AMINO ACID TAB",kind.name(),path);
    }
    fn virus_click_control(&mut self,button:usize) {
        match button {
            0=>{self.virus_experimental=false;self.status="EDUCATIONAL VIRION: ALL COMPONENT PLACEMENTS ARE ILLUSTRATIVE".into();},
            1=>{self.virus_experimental=true;self.status="EXPERIMENTAL VIEW: ORIGINAL EM VOXELS; NO INFERRED PROTEIN PLACEMENTS".into();},
            2=>self.virus.cutaway=!self.virus.cutaway,
            3=>self.open_dialog(),
            4=>self.attach_active_protein(),
            5=>self.inspect_virus_protein(),
            6=>self.reset_camera(),
            7=>{self.virus.visible=[true;6];self.virus.cutaway=true;self.status="VIRUS COMPONENT VISIBILITY RESET".into();},
            8=>self.virus_mode=VolumeMode::Surface,
            9=>self.virus_mode=VolumeMode::MaxIntensity,
            _=>{},
        }
    }
    fn virus_handle_click(&mut self,x:i32,y:i32) {
        if let Some(index)=self.virus_buttons().iter().position(|r|r.hit(x,y)) {self.virus_click_control(index);return;}
        let slider=self.virus_contour_slider();
        if self.virus_experimental && self.virus_mode==VolumeMode::Surface && self.virus_density.is_some()
            && Rect::new(slider.x,slider.y-15,slider.w,31).hit(x,y) {
            self.drag_virus_contour=true;
            self.virus_threshold=((x-slider.x) as f32/slider.w.max(1) as f32).clamp(0.001,0.999);
            return;
        }
        let sidebar=self.sidebar_rect();
        if sidebar.hit(x,y) {
            if self.virus_experimental{return;}
            let row=(y-sidebar.y-90)/25;
            if y>=sidebar.y+90 && (0..6).contains(&row) {
                let component=ViralComponent::ALL[row as usize];
                if x<sidebar.x+50 {self.virus.toggle(component);}
                else {self.virus_selected=Some(component);}
            }return;
        }
        let image=self.virus_image_rect();
        if image.hit(x,y) {
            if self.virus_experimental {
                if let Some(index)=self.virus_density {
                    if let Some(CellData::Microscopy(volume))=self.cell_data.get(index) {
                        self.cell_probe=CellViewer::inspect_voxel(volume,image,x,y,VolumeMode::MaxIntensity,0.5,self.ax,self.ay,self.zoom,self.pan);
                    }
                }
            } else if let Some(kind)=VirusViewer::pick(&self.virus,image,x,y,self.ax,self.ay,self.zoom,self.pan) {
                self.virus_selected=Some(kind);
            }
            self.drag_rotate=true;
        }
    }
    fn draw_virus(&mut self) {
        let area=self.canvas_rect();let side=self.sidebar_rect();self.panel(area);self.panel(side);
        let buttons=self.virus_buttons();
        let labels=["EDUCATIONAL","EXPERIMENTAL","CUTAWAY","IMPORT FILE","LINK ACTIVE","INSPECT PROTEIN","RESET VIEW","SHOW ALL","3D SURFACE","3D MIP"];
        for (i,r) in buttons.into_iter().enumerate(){
            self.button(r,labels[i],match i {0=>!self.virus_experimental,1=>self.virus_experimental,2=>self.virus.cutaway,8=>self.virus_experimental&&self.virus_mode==VolumeMode::Surface,9=>self.virus_experimental&&self.virus_mode==VolumeMode::MaxIntensity,_=>false});
        }
        let line=self.virus_contour_slider();
        if self.virus_experimental {
            if let Some(CellData::Microscopy(vol))=self.virus_density.and_then(|i|self.cell_data.get(i)) {
                let (lo,hi)=vol.min_max();
                let label=if self.virus_mode==VolumeMode::Surface {
                    format!("CONTOUR: {:.4} ({}%) | ADJUST TO REVEAL THE VIRION",lo+(hi-lo)*self.virus_threshold,self.virus_threshold.mul_add(100.0,0.0).round())
                }else {"3D MIP: MAXIMUM INTENSITY PROJECTION | SWITCH TO SURFACE TO REMOVE BACKGROUND".into()};
                draw::text_clip(&mut self.fb,line.x,line.y-14,&label,1,WHITE,(line.w/6).max(1) as usize);
                if self.virus_mode==VolumeMode::Surface {
                    draw::fill(&mut self.fb,line,0xff334963);
                    draw::fill(&mut self.fb,Rect::new(line.x,line.y,(line.w as f32*self.virus_threshold) as i32,9),BLUE);
                    draw::circle(&mut self.fb,line.x+(line.w as f32*self.virus_threshold) as i32,line.y+4,8,BLUE);
                }
            }
        }
        let rect=self.virus_image_rect();
        if !self.virus_experimental {
            self.virus_viewer.draw(&mut self.fb,rect,&self.virus,self.virus_selected,self.ax,self.ay,self.zoom,self.pan);
            #[cfg(target_os="windows")]
            if self.gpu_enabled {
                if let (Some(layer),Some((r,_)))=(self.d3d11.as_mut(),self.virus_viewer.gpu_scene()) {
                    layer.capture_ui_underlay(&self.fb,r);
                }
            }
            draw::text_clip(&mut self.fb,rect.x+10,rect.y+7,"EDUCATIONAL: SCHEMATIC COMPONENT LOCATIONS",1,GOLD,((rect.w-20)/6).max(1) as usize);
        }else if matches!(self.virus_density.and_then(|i|self.cell_data.get(i)),Some(CellData::Microscopy(_))) {
            // Borrow the imported volume in place; never duplicate large voxel arrays per frame.
            self.cell_viewer.draw(&mut self.fb,rect,&self.cell_data[self.virus_density.unwrap()],self.ax,self.ay,self.zoom,self.pan,None,self.virus_mode,0.5,if self.virus_mode==VolumeMode::Surface{self.virus_threshold}else{1.0});
            draw::text_clip(&mut self.fb,rect.x+10,rect.y+7,if self.virus_mode==VolumeMode::Surface {"EXPERIMENTAL DENSITY: THRESHOLDED SURFACE (ESTIMATED CONTOUR)"}else{"EXPERIMENTAL DENSITY: MAXIMUM INTENSITY PROJECTION"},1,BLUE,((rect.w-20)/6).max(1) as usize);
        }else {
            draw::centered(&mut self.fb,rect.x+rect.w/2,rect.y+rect.h/2-15,"NO EXPERIMENTAL VIRION DENSITY LOADED",1,GOLD);
            draw::centered(&mut self.fb,rect.x+rect.w/2,rect.y+rect.h/2+8,"IMPORT AN MRC / MAP / NRRD VOLUME",1,MUTED);
        }
        draw::text(&mut self.fb,side.x+13,side.y+14,"VIRUS EXPLORER",2,WHITE);
        draw::text_clip(&mut self.fb,side.x+13,side.y+46,if self.virus_experimental{"REAL DATA / EXPERIMENTAL"}else{"SARS-COV-2 / SCHEMATIC"},1,GOLD,((side.w-24)/6).max(1) as usize);
        draw::text_clip(&mut self.fb,side.x+13,side.y+68,"EYE: TOGGLE  |  NAME: SELECT",1,MUTED,((side.w-24)/6).max(1) as usize);
        if !self.virus_experimental {for (i,kind) in ViralComponent::ALL.iter().enumerate() {
            let y=side.y+90+i as i32*25;
            if self.virus_selected==Some(*kind) {draw::fill(&mut self.fb,Rect::new(side.x+9,y-2,side.w-18,26),0xff294157);}
            draw::circle(&mut self.fb,side.x+23,y+9,7,if self.virus.is_visible(*kind){kind.color()}else{0xff48596b});
            draw::text_clip(&mut self.fb,side.x+40,y+4,kind.name(),1,if self.virus.is_visible(*kind){WHITE}else{MUTED},((side.w-55)/6).max(1) as usize);
        }
        }
        let details_y=side.y+side.h-87;
        draw::fill(&mut self.fb,Rect::new(side.x+8,details_y-5,side.w-16,88),0xff192d41);
        if !self.virus_experimental {if let Some(kind)=self.virus_selected {
            draw::text_clip(&mut self.fb,side.x+15,details_y,&format!("{} | {} schematic glyphs",kind.name(),self.virus.count(kind)),1,GOLD,((side.w-24)/6).max(1) as usize);
            draw::text_clip(&mut self.fb,side.x+15,details_y+18,kind.description(),1,MUTED,((side.w-24)/6).max(1) as usize);
            let link=self.virus_links.get(&kind).map(|s|s.as_str()).unwrap_or("NO LINKED ATOMIC STRUCTURE");
            draw::text_clip(&mut self.fb,side.x+15,details_y+36,link,1,BLUE,((side.w-24)/6).max(1) as usize);
            draw::text_clip(&mut self.fb,side.x+15,details_y+54,kind.reference(),1,MUTED,((side.w-24)/6).max(1) as usize);
        }
        }
        if self.virus_experimental {
            if let Some(CellData::Microscopy(vol))=self.virus_density.and_then(|i|self.cell_data.get(i)) {
                let max=((side.w-26)/6).max(1) as usize;
                let (lo,hi)=vol.min_max();
                draw::text_clip(&mut self.fb,side.x+13,side.y+96,&vol.name,1,WHITE,max);
                draw::text_clip(&mut self.fb,side.x+13,side.y+120,&format!("{} x {} x {} VOXELS",vol.dims[0],vol.dims[1],vol.dims[2]),1,BLUE,max);
                draw::text_clip(&mut self.fb,side.x+13,side.y+143,&format!("RAW DENSITY {:.4} TO {:.4}",lo,hi),1,MUTED,max);
                draw::text_clip(&mut self.fb,side.x+13,side.y+166,"SURFACE = THRESHOLDED DENSITY",1,WHITE,max);
                draw::text_clip(&mut self.fb,side.x+13,side.y+188,"MIP = PROJECTION THROUGH DATA",1,MUTED,max);
                draw::text_clip(&mut self.fb,side.x+13,side.y+220,"MASKS SHOW REGIONS, NOT DETAILS",1,GOLD,max);
                draw::text_clip(&mut self.fb,side.x+13,details_y,"NO AUTOMATIC PROTEIN LABELS",1,GOLD,max);
                draw::text_clip(&mut self.fb,side.x+13,details_y+20,"LOAD PDB/MMCIF FOR ATOMIC VIEW",1,MUTED,max);
            }
        }
        if self.virus_experimental && self.virus_density.is_some() {
            draw::text_clip(&mut self.fb,rect.x+10,rect.y+27,"SURFACE/MIP SHOW DENSITY, NOT ATOMIC COORDINATES",1,GOLD,((rect.w-20)/6).max(1) as usize);
            if let Some((loc,value))=self.cell_probe {
                draw::text_clip(&mut self.fb,rect.x+10,rect.y+48,&format!("VOXEL ({},{},{}) INTENSITY {:.3}",loc[0],loc[1],loc[2],value),1,WHITE,((rect.w-20)/6).max(1) as usize);
            }
        }
    }
    fn cell_selected_data(&self)->Option<&CellData> {self.cell_data.get(self.cell_index)}
    fn cell_choose(&mut self,index:usize){
        if index>=self.cell_data.len(){return;}
        self.cell_viewer.invalidate();
        self.cell_index=index;self.cell_selected=if let Some(CellData::Generated(c))=self.cell_selected_data(){
            c.organelles.iter().position(|o|o.kind==mata_molecule::cell::OrganelleKind::Nucleus).or(Some(0))
        }else{None};
        self.cell_slice=0.5;self.cell_gain=1.0;
        self.cell_threshold=match self.cell_selected_data(){Some(CellData::Microscopy(vol))=>vol.suggested_contour(),_=>0.5};
        self.cell_scroll=0;self.cell_probe=None;self.reset_camera();
    }
    fn cell_click_control(&mut self,index:usize){
        match index {
            0=>self.cell_choose(0),1=>self.cell_choose(1),
            2=>self.open_dialog(),
            3=>{if self.cell_index>0{self.cell_choose(self.cell_index-1)}},
            4=>{if self.cell_index+1<self.cell_data.len(){self.cell_choose(self.cell_index+1)}},
            5=>self.cell_mode=VolumeMode::MaxIntensity,
            6=>self.cell_mode=VolumeMode::Xy,
            7=>self.cell_mode=VolumeMode::Xz,
            8=>self.cell_mode=VolumeMode::Yz,
            9=>self.reset_camera(),10=>self.cell_mode=VolumeMode::Surface,_=>{}
        }
    }
    fn cell_handle_click(&mut self,x:i32,y:i32){
        if let Some(i)=self.cell_buttons().iter().position(|r|r.hit(x,y)) {self.cell_click_control(i);return;}
        let slider=self.cell_slider();
        if (matches!(self.cell_selected_data(),Some(CellData::Microscopy(_)))) && Rect::new(slider.x,slider.y-15,slider.w,32).hit(x,y) {
            self.drag_cell_slice=true;
            let t=((x-slider.x) as f32/slider.w.max(1) as f32).clamp(0.0,1.0);
            if self.cell_mode==VolumeMode::Surface {self.cell_threshold=t.clamp(0.001,0.999)} else {self.cell_slice=t}
            return;
        }
        let side=self.sidebar_rect();if side.hit(x,y) {
            if self.cell_index>=2 && Rect::new(side.x+side.w-91,side.y+8,80,27).hit(x,y) {
                self.cell_data.remove(self.cell_index);
                self.cell_choose(self.cell_data.len()-1);
                self.status="CLOSED MICROSCOPY DATASET (SOURCE FILE UNCHANGED)".into();return;
            }
            if let Some(CellData::Generated(cell))=self.cell_selected_data(){
                if y>=side.y+105 {
                    let row=(y-side.y-105)/26;
                    if row>=0 {
                        let selected=self.cell_scroll+row as usize;
                        if selected<cell.organelles.len() {
                            if x<side.x+53 {if let Some(CellData::Generated(c))=self.cell_data.get_mut(self.cell_index){c.organelles[selected].visible=!c.organelles[selected].visible;}}
                            else {self.cell_selected=Some(selected);}
                        }
                    }
                }
            }return;
        }
        if self.cell_image_rect().hit(x,y) {
            if let Some(CellData::Generated(c))=self.cell_selected_data(){
                if let Some(index)=CellViewer::pick_organelle(c,self.cell_image_rect(),x,y,self.ax,self.ay,self.zoom,self.pan){self.cell_selected=Some(index);}
            }
            if let Some(CellData::Microscopy(v))=self.cell_selected_data() {
                self.cell_probe=CellViewer::inspect_voxel(v,self.cell_image_rect(),x,y,self.cell_mode,self.cell_slice,self.ax,self.ay,self.zoom,self.pan);
            }
            self.drag_rotate=true;
        }
    }
    fn draw_cells(&mut self){
        let canvas=self.canvas_rect();let side=self.sidebar_rect();
        self.panel(canvas);self.panel(side);
        let buttons=self.cell_buttons();
        let labels=["ANIMAL CELL","PLANT CELL","OPEN VOLUME","PREVIOUS","NEXT","3D MIP","XY","XZ","YZ","RESET VIEW","3D SURFACE"];
        for (i,r) in buttons.iter().enumerate(){self.button(*r,labels[i],match i {0=>self.cell_index==0,1=>self.cell_index==1,5=>self.cell_mode==VolumeMode::MaxIntensity,6=>self.cell_mode==VolumeMode::Xy,7=>self.cell_mode==VolumeMode::Xz,8=>self.cell_mode==VolumeMode::Yz,10=>self.cell_mode==VolumeMode::Surface,_=>false});}
        let line=self.cell_slider();
        let volume=matches!(self.cell_selected_data(),Some(CellData::Microscopy(_)));
        if volume {
            let pct=if self.cell_mode==VolumeMode::Surface {self.cell_threshold}else{self.cell_slice};
            let label=if self.cell_mode==VolumeMode::Surface {
                let raw=self.cell_selected_data().and_then(|d|if let CellData::Microscopy(v)=d{Some(v.min_max())}else{None}).map_or(0.,|(lo,hi)|lo+(hi-lo)*self.cell_threshold);
                format!("3D SURFACE | CONTOUR {:.4} | MOVE SLIDER TO REMOVE BACKGROUND",raw)
            }else if self.cell_mode==VolumeMode::MaxIntensity {
                "3D MIP | ALL INTENSITIES PROJECTED | SELECT SURFACE FOR A SHAPE".into()
            }else{
                format!("SLICE {:3.0}% | {}",self.cell_slice*100.0,self.cell_mode.label())
            };
            draw::text_clip(&mut self.fb,line.x,line.y-14,&label,1,WHITE,((line.w)/6).max(1) as usize);
            if self.cell_mode!=VolumeMode::MaxIntensity {
                draw::fill(&mut self.fb,line,0xff334963);
                draw::fill(&mut self.fb,Rect::new(line.x,line.y,(line.w as f32*pct) as i32,9),BLUE);
                draw::circle(&mut self.fb,line.x+(line.w as f32*pct) as i32,line.y+4,8,BLUE);
            }
        }else {draw::text_clip(&mut self.fb,line.x,line.y-14,"ILLUSTRATIVE MODEL - NOT A MICROSCOPY RECONSTRUCTION",1,GOLD,((line.w)/6).max(1) as usize);}
        let content=self.cell_image_rect();
        if content.h>3 {
            if let Some(data)=self.cell_data.get(self.cell_index) {
                self.cell_viewer.draw(&mut self.fb,content,data,self.ax,self.ay,self.zoom,self.pan,self.cell_selected,self.cell_mode,self.cell_slice,if self.cell_mode==VolumeMode::Surface{self.cell_threshold}else{self.cell_gain});
                #[cfg(target_os="windows")]
                if self.gpu_enabled {
                    if let (Some(layer),Some((r,_)))=(self.d3d11.as_mut(),self.cell_viewer.gpu_scene()) {
                        layer.capture_ui_underlay(&self.fb,r);
                    }
                }
            }
        }
        let max=((side.w-36)/6).max(1) as usize;
        draw::text(&mut self.fb,side.x+16,side.y+15,"CELL EXPLORER",2,WHITE);
        if self.cell_index>=2 {self.button(Rect::new(side.x+side.w-91,side.y+8,80,27),"CLOSE",false);}
        if let Some(data)=self.cell_data.get(self.cell_index){
            draw::text_clip(&mut self.fb,side.x+16,side.y+51,data.name(),1,GOLD,max);
            match data {
                CellData::Generated(cell)=>{
                    draw::text_clip(&mut self.fb,side.x+16,side.y+70,&format!("{:0.1} x {:0.1} x {:0.1} um",cell.size_um.x,cell.size_um.y,cell.size_um.z),1,BLUE,max);
                    draw::text(&mut self.fb,side.x+16,side.y+90,"EYE TO SHOW/HIDE  |  NAME TO INSPECT",1,MUTED);
                    let rows=((side.h-135)/26).max(1) as usize;
                    for i in self.cell_scroll..(self.cell_scroll+rows).min(cell.organelles.len()) {
                        let y=side.y+105+((i-self.cell_scroll)*26) as i32;
                        let org=&cell.organelles[i];
                        if self.cell_selected==Some(i){draw::fill(&mut self.fb,Rect::new(side.x+12,y-2,side.w-24,24),0xff29435b);}
                        draw::circle(&mut self.fb,side.x+26,y+8,7,if org.visible{org.kind.color()}else{0xff3d5266});
                        draw::text_clip(&mut self.fb,side.x+43,y+4,&org.name,1,if org.visible{WHITE}else{MUTED},((side.w-53)/6).max(1) as usize);
                    }
                    if let Some(org)=self.cell_selected.and_then(|i|cell.organelles.get(i)) {
                        let base=side.y+side.h-64;
                        draw::fill(&mut self.fb,Rect::new(side.x+10,base-5,side.w-20,62),0xff1b3149);
                        draw::text_clip(&mut self.fb,side.x+17,base,&format!("{} (SIMULATED)",org.kind.label()),1,GOLD,max);
                        draw::text_clip(&mut self.fb,side.x+17,base+20,org.kind.function(),1,MUTED,max);
                        draw::text_clip(&mut self.fb,side.x+17,base+37,&format!("Size approx {:.1} x {:.1} x {:.1} um",org.radii.x*2.0,org.radii.y*2.0,org.radii.z*2.0),1,BLUE,max);
                    }
                },
                CellData::Microscopy(vol)=>{
                    draw::text_clip(&mut self.fb,side.x+16,side.y+84,&format!("{} x {} x {} VOXELS",vol.dims[0],vol.dims[1],vol.dims[2]),1,BLUE,max);
                    draw::text_clip(&mut self.fb,side.x+16,side.y+105,&format!("FORMAT: {}",vol.source_format),1,MUTED,max);
                    let d=vol.dimensions_um();
                    if vol.calibrated {draw::text_clip(&mut self.fb,side.x+16,side.y+129,&format!("SIZE: {:.2} x {:.2} x {:.2} um",d[0],d[1],d[2]),1,WHITE,max);}
                    else {draw::text_clip(&mut self.fb,side.x+16,side.y+129,"PIXEL SCALE UNKNOWN",1,GOLD,max);}
                    let (lo,hi)=vol.min_max();
                    draw::text_clip(&mut self.fb,side.x+16,side.y+153,&format!("INTENSITY: {:.2} TO {:.2}",lo,hi),1,MUTED,max);
                    draw::text_clip(&mut self.fb,side.x+16,side.y+197,"SURFACE = DENSITY ABOVE CONTOUR",1,BLUE,max);
                    draw::text_clip(&mut self.fb,side.x+16,side.y+219,"MIP / XY-XZ-YZ ALSO AVAILABLE",1,MUTED,max);
                    draw::text_clip(&mut self.fb,side.x+16,side.y+242,"NO AUTOMATIC ORGANELLE LABELS",1,GOLD,max);
                    if let Some((loc,val))=self.cell_probe {
                        draw::text_clip(&mut self.fb,side.x+16,side.y+270,&format!("VOXEL ({}, {}, {})",loc[0],loc[1],loc[2]),1,BLUE,max);
                        draw::text_clip(&mut self.fb,side.x+16,side.y+290,&format!("INTENSITY {:.4}",val),1,WHITE,max);
                    }
                }
            }
        }
    }
    fn reset_camera(&mut self) {
        self.ax=-0.35;self.ay=0.48;self.zoom=1.0;self.pan=(0.0,0.0);
        self.drag_rotate=false;self.drag_pan=false;self.camera_motion.reset();
    }
    fn select_chain(&mut self,chain:String) {
        if self.focused_chain.as_deref()==Some(chain.as_str()) {
            self.focused_chain=None;
            self.status="SHOWING ALL PROTEIN CHAINS".into();
        } else {
            self.focused_chain=Some(chain.clone());self.active_visible=true;
            self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;
            self.status=format!("FOCUSED CHAIN {chain} | COLORS = AMINO ACID CLASS; BONDS = FULL CONNECTIVITY");
        }
        if let Some(chain)=self.focused_chain.as_ref() {
            if self.selected_residue.and_then(|i|self.residues.get(i))
                .is_none_or(|r|r.key.chain!=*chain) {
                self.selected_residue=self.residues.iter().position(|r|r.key.chain==*chain);
            }
        }
        self.reset_camera();
    }
    /// In a protein, a residue *is* one amino acid. Clicking it isolates the
    /// amino acid; clicking again selects a particular atom and its bonds.
    fn focus_on_residue(&mut self,index:usize) {
        if index>=self.residues.len(){return;}
        self.selected_residue=Some(index);
        self.focus_residue=true;
        self.selected_atom=None;
        self.show_electrons=false;
        self.electron_neighbor_scroll=0;
        self.focused_chain=None;
        self.active_visible=true;
        self.inspector_open=true;
        self.detail_mode=view::DetailMode::AllAtoms;
        self.zoom=1.3;
        self.pan=(0.0,0.0);
        self.camera_motion.reset();
        self.status=format!("FOCUSED {} | CLICK AN ATOM TO INSPECT ITS BONDED NEIGHBORS",
            self.residues[index].key.display());
    }
    fn selected_residue_atoms(&self)->Vec<usize> {
        let Some(res)=self.selected_residue.and_then(|i|self.residues.get(i))else{return Vec::new();};
        self.display_current().map_or_else(Vec::new,|m|m.atoms.iter().enumerate()
            .filter_map(|(i,a)|a.site.as_ref().and_then(|s|
                (s.chain_id==res.key.chain && s.residue_number==res.key.number &&
                 s.insertion_code==res.key.insertion && s.residue_name==res.key.name)
                    .then_some(i)))
            .collect())
    }
    fn model_scene_rect(&self)->Rect {
        let canvas=self.canvas_rect();
        let top=self.model_toolbar_bottom()+9;
        Rect::new(canvas.x+1,top,(canvas.w-2).max(2),(canvas.y+canvas.h-top-2).max(2))
    }
    fn pick_model(&mut self,x:i32,y:i32) {
        if !self.active_visible || self.tab!=Tab::Model3d{return;}
        let members=self.selected_residue_atoms();
        let focused=self.focus_residue && !members.is_empty();
        let atom=if let Some(model)=self.display_current(){
            let extra=self.other_files.iter().filter(|file|file.visible)
                .filter_map(|file|file.preview.as_ref().map(|p|&p.molecule)
                    .or_else(||file.models.get(file.index)))
                .collect::<Vec<_>>();
            view::pick_atom(model,self.model_scene_rect(),x,y,self.ax,self.ay,
                self.zoom,self.pan,self.focused_chain.as_deref(),
                if focused{Some(members.as_slice())}else{None},
                if focused{self.selected_atom}else{None},self.detail_mode,&extra)
        }else{None};
        let Some(atom_id)=atom else{return;};
        if focused {
            if !members.contains(&atom_id){return;}
            self.selected_atom=Some(atom_id);
            self.show_electrons=false;
            self.electron_neighbor_scroll=0;
            self.zoom=self.zoom.max(1.5);
            self.pan=(0.0,0.0);
            let Some(m)=self.display_current()else{return;};
            let name=m.atoms[atom_id].site.as_ref().map_or("",|s|s.atom_name.as_str());
            self.status=format!("SELECTED ATOM {} ({}) | SHOW ELECTRON CLOUDS IN THE RIGHT DOCK",
                name,m.atoms[atom_id].symbol);
        }else if let Some(site)=self.display_current().and_then(|m|m.atoms.get(atom_id))
                .and_then(|a|a.site.as_ref()) {
            if let Some(res_index)=self.residues.iter().position(|r|
                r.key.chain==site.chain_id && r.key.number==site.residue_number &&
                r.key.insertion==site.insertion_code && r.key.name==site.residue_name){
                self.focus_on_residue(res_index);
            }
        }
    }
    fn electron_toggle_rect(&self)->Rect {
        let r=self.sidebar_rect();
        Rect::new(r.x+15,r.y+192,(r.w-30).max(30),36)
    }
    fn orbital_filter_rect(&self,index:usize)->Rect {
        let r=self.sidebar_rect();
        let w=((r.w-45)/4).max(15);
        Rect::new(r.x+15+index as i32*(w+5),r.y+237,w,29)
    }
    fn draw_residue_inspector(&mut self,r:Rect) {
        self.panel(r);
        let Some(res)=self.selected_residue.and_then(|i|self.residues.get(i)) else{return;};
        let res_title=res.key.display();
        let aa_name=protein_inspect::full_name(&res.key.name).to_owned();
        let atoms=self.selected_residue_atoms();
        let selected=self.selected_atom.and_then(|i|self.display_current()
            .and_then(|m|m.atoms.get(i).map(|a|{
                let label=a.site.as_ref().map_or_else(||format!("ATOM {}",i+1),|s|s.atom_name.clone());
                let configuration=electron::configuration_short_label(&a.symbol);
                let neighbors=electron::bonded_atoms(m,i).into_iter().filter(|&id|id!=i)
                    .map(|id|{
                        let a=&m.atoms[id];
                        let name=a.site.as_ref().map_or_else(||format!("#{}",id+1),|site|site.atom_name.clone());
                        let order=m.bonds.iter().find(|b|(b.a==i&&b.b==id)||(b.a==id&&b.b==i))
                            .map_or(1,|b|if m.apply_templates {protein_inspect::display_bond_order(m,b)} else{b.order});
                        (name,a.symbol.clone(),order,electron::configuration_short_label(&a.symbol))
                    }).collect::<Vec<_>>();
                (label,a.symbol.clone(),a.charge,configuration,neighbors)
            })));
        let max=((r.w-30)/6).max(1) as usize;
        draw::text_clip(&mut self.fb,r.x+16,r.y+18,"FOCUSED AMINO ACID",1,BLUE,max);
        draw::text_clip(&mut self.fb,r.x+16,r.y+47,&aa_name.to_ascii_uppercase(),2,WHITE,((r.w-32)/12).max(1) as usize);
        draw::text_clip(&mut self.fb,r.x+16,r.y+80,&res_title,1,GOLD,max);
        draw::text_clip(&mut self.fb,r.x+16,r.y+105,&format!("{} ATOMS | LEFT CLICK TO SELECT",atoms.len()),1,MUTED,max);
        draw::fill(&mut self.fb,Rect::new(r.x+15,r.y+136,r.w-30,1),STROKE);
        if let Some((name,symbol,charge,config,neighbors))=selected {
            draw::text_clip(&mut self.fb,r.x+16,r.y+150,&format!("ATOM: {} ({})",name,symbol),1,WHITE,max);
            draw::text_clip(&mut self.fb,r.x+16,r.y+170,&format!("FORMAL CHARGE: {:+}",charge),1,MUTED,max);
            let button=self.electron_toggle_rect();
            self.button(button,if self.show_electrons{"HIDE ELECTRONS"}else{"SHOW ELECTRONS"},self.show_electrons);
            // S/P/D/F are independent filters; only occupied orbitals are
            // drawn. Empty masks automatically suppress all electron clouds.
            for (i,label) in ["S","P","D","F"].iter().enumerate() {
                let active=self.orbital_mask&(1u8<<i)!=0;
                self.button(self.orbital_filter_rect(i),label,active);
            }
            draw::text_clip(&mut self.fb,r.x+16,r.y+280,"NEUTRAL ATOMIC CONFIGURATION",1,BLUE,max);
            draw::text_clip(&mut self.fb,r.x+16,r.y+299,&config,1,WHITE,max);
            draw::text_clip(&mut self.fb,r.x+16,r.y+329,&format!("{} BONDED ATOM(S)",neighbors.len()),1,GOLD,max);
            let count=neighbors.len();
            let visible=((r.h-423)/36).max(1) as usize;
            let scroll=self.electron_neighbor_scroll.min(count.saturating_sub(visible));
            for (n,(name,symbol,order,configuration)) in neighbors.into_iter()
                .skip(scroll).take(visible).enumerate() {
                let y=r.y+351+n as i32*36;
                draw::text_clip(&mut self.fb,r.x+16,y,
                    &format!("{} ({}) BOND {}",name,symbol,if order==4{"AR".into()}else{order.to_string()}),
                    1,WHITE,max);
                draw::text_clip(&mut self.fb,r.x+16,y+16,&configuration,1,MUTED,max);
            }
            if count>visible {draw::text_clip(&mut self.fb,r.x+16,r.y+r.h-75,
                "SCROLL HERE FOR MORE BONDED ATOMS",1,BLUE,max);}
        }else {
            draw::text_clip(&mut self.fb,r.x+16,r.y+163,"CLICK AN ATOM IN THE VIEWER",1,WHITE,max);
            draw::text_clip(&mut self.fb,r.x+16,r.y+187,"TO INSPECT ITS BOND CLUSTER",1,MUTED,max);
        }
        let note_y=r.y+r.h-54;
        if note_y>r.y+350 {
            draw::fill(&mut self.fb,Rect::new(r.x+15,note_y-11,r.w-30,1),STROKE);
            for (line,text) in [
                "S CYAN | P VIOLET | D GREEN",
                "F ORANGE | GOLD: BOND MARKERS",
                "NEUTRAL ATOM, NOT MOLECULAR QM",
            ].iter().enumerate() {
                draw::text_clip(&mut self.fb,r.x+16,note_y+line as i32*15,text,1,MUTED,max);
            }
        }
    }
    fn residue_buttons(&self)->(Rect,Rect){let r=self.report_rect();
        (Rect::new(r.x+r.w-410,r.y+16,174,36),Rect::new(r.x+r.w-220,r.y+16,190,36))
    }
    fn files_button(&self)->Rect{let r=self.report_rect();Rect::new(r.x+r.w-190,r.y+15,160,35)}

    fn fetch_rect(&self)->Rect {
        let r=if self.tab==Tab::Record{self.report_rect()}else{self.canvas_rect()};
        if self.tab==Tab::Record {Rect::new(r.x+r.w-175,r.y+74,153,33)}
        else {Rect::new(r.x+(r.w-180)/2,r.y+(r.h/2)+31,180,40)}
    }
    fn start_protein_fetch(&mut self) {
        if self.pending_model.is_some() {return;}
        let Some(record)=self.record.as_ref() else{return;};
        if !record.record_type.eq_ignore_ascii_case("protein") {
            self.status="3D DOWNLOAD ERROR: Only protein accessions can be looked up in AlphaFold".into();return;
        }
        if !protein_structure::valid_accession(&record.accession) {
            self.status="3D DOWNLOAD ERROR: No usable UniProt accession; open a PDB/mmCIF model".into();return;
        }
        let accession=record.accession.clone();
        let (tx,rx)=mpsc::channel();
        let query=accession.clone();
        std::thread::spawn(move||{
            let result=protein_structure::fetch_prediction(&query);
            let _=tx.send(result);
        });
        self.pending_model=Some((accession.clone(),rx));
        self.status=format!("LOOKING UP {accession} IN ALPHAFOLD DATABASE (predicted coordinates)...");
    }
    fn poll_protein_fetch(&mut self)->bool {
        let Some((accession,rx))=self.pending_model.as_ref() else{return false;};
        let Ok(response)=rx.try_recv() else{return false;};
        let active=self.record.as_ref().is_some_and(|r|r.accession==*accession);
        self.pending_model=None;
        if !active {return false;}
        match response {
            Ok((models,source))=>{
                let atom_count=models.first().map_or(0,|m|m.atoms.len());
                self.models=models;self.index=0;self.vsepr_scroll=0;self.cache_residues();
                // Preserve the original ASNT record; Save As must never
                // default to overwriting the sequence/annotation file.
                if let Some(record)=self.record.as_ref(){
                    let original=std::path::Path::new(&self.path);
                    let suggested=format!("{}-alphafold.cif",record.accession);
                    self.path=original.with_file_name(suggested).to_string_lossy().into_owned();
                    self.file_input=self.path.clone();
                }
                self.formula=self.models.first().map_or(String::new(),|m|m.formula());
                self.tab=Tab::Model3d;self.reset_camera();self.focused_chain=None;
                self.status=format!("{source} | {} ATOMS | {}",atom_count,
                    if self.models.first().is_some_and(view::is_protein) {"C-ALPHA BACKBONE"} else {"BALL AND STICK"});
            },
            Err(e)=>self.status=format!("3D DOWNLOAD ERROR: {e}"),
        }
        true
    }
    fn set_variant_from_x(&mut self,x:i32){let r=self.slider_rect();let count=self.models.len();if count<2{return;}
        let percent=((x-r.x) as f32/r.w.max(1) as f32).clamp(0.0,1.0);self.index=((count-1) as f32*percent).round() as usize;
        self.reset_camera();self.focused_chain=None;self.vsepr_scroll=0;self.cache_residues();
    }
    fn recalc(&mut self){self.initial_draft=false;self.pending_model=None;match propose(&self.formula){
        Ok(models)=>{if self.record.is_some(){self.path="molecule.sdf".into();self.file_input=self.path.clone();self.tab=Tab::Lewis;}self.record=None;self.models=models;self.index=0;self.reset_camera();self.focused_chain=None;self.vsepr_scroll=0;self.cache_residues();self.status=format!("GENERATED {} candidate structure(s). Formula alone does not determine connectivity.",self.models.len());},
        Err(e)=>{self.status=format!("FORMULA ERROR: {e}");}
    }}
    fn load(&mut self){self.open_path(self.file_input.clone());}
    fn open_path(&mut self,target:String){
        let was_virus=self.tab==Tab::Virus;
        let path=std::path::Path::new(&target);
        let ext=path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
        if ["mrc","map","ccp4","nrrd","tif","tiff"].contains(&ext.as_str()) {
            if self.cell_data.len()>=8 {self.status="CELL DATA LIMIT: REMOVE A MICROSCOPY DATASET FIRST".into();return;}
            match load_volume(path) {
                Ok(volume)=>{
                    self.cell_data.push(CellData::Microscopy(volume));
                    self.cell_choose(self.cell_data.len()-1);
                    if was_virus {
                        self.virus_density=Some(self.cell_index);
                        self.virus_threshold=self.cell_threshold;
                        self.virus_mode=VolumeMode::Surface;
                        self.virus_experimental=true;self.tab=Tab::Virus;
                    } else {
                        self.cell_mode=VolumeMode::Surface;
                        self.tab=Tab::Cell;
                    }
                    self.status=format!("OPENED MICROSCOPY VOLUME: {target} | DATA UNMODIFIED");
                },Err(e)=>self.status=format!("VOLUME LOAD ERROR: {e}"),
            }return;
        }
        if self.file_count()>=12 && !self.initial_draft {
            self.status="FILE LIMIT: at most 12 loaded files; remove unused structures first".into();return;
        }
        match read_document(&target){
            Ok(OpenedDocument::Structures(mut molecules))=>{
                for m in &mut molecules {if m.atoms.iter().all(|a|a.pos.len()<0.0001){layout3d(m);}}
                let count=molecules.len();
                let residue_list=molecules.first().map(protein_inspect::residues).unwrap_or_default();
                self.add_file(LoadedFile{path:target.clone(),models:molecules,index:0,visible:true,record:None,
                    selected_residue:if residue_list.is_empty(){None}else{Some(0)},residues:residue_list,preview:None});
                if self.tab==Tab::Record {self.tab=Tab::Model3d;}
                if was_virus {
                    self.tab=Tab::Virus;
                    if self.current().is_some_and(|m|m.atoms.iter().any(|a|a.site.is_some())) {self.attach_active_protein();}
                }
                self.status=format!("OPENED {count} MODEL(S) IN {} | {} FILE(S) LOADED",target,self.file_count());
            },
            Ok(OpenedDocument::ProteinRecord(record))=>{
                let count=record.sequence.len();let accession=record.accession.clone();
                self.add_file(LoadedFile{path:target,models:Vec::new(),index:0,visible:true,record:Some(record),
                    selected_residue:None,residues:Vec::new(),preview:None});
                self.tab=Tab::Record;self.focus=Focus::Neither;
                self.status=format!("OPENED {accession} | {count} AMINO ACIDS | LOAD 3D MODEL FROM RECORD TAB");
            },
            Err(e)=>self.status=format!("LOAD ERROR: {e}"),
        }
    }
    fn open_dialog(&mut self) {
        match file_dialog::pick_many(std::path::Path::new(&self.file_input),self.window.handle()) {
            Ok(Some(paths))=>{
                self.focus=Focus::Neither;
                let mut attempted=0usize;
                for path in paths {attempted+=1;self.open_path(path.to_string_lossy().into_owned());}
                if attempted>1 && !self.status.contains("ERROR"){
                    self.status=format!("IMPORTED {} FILE(S) | {} LOADED",attempted,self.file_count());
                }
            },
            Ok(None)=>{},
            Err(err)=>self.status=format!("OPEN DIALOG ERROR: {err}"),
        }
    }
    fn save_dialog(&mut self) {
        if self.record.is_some() && self.models.is_empty() { self.status = "The PubChem information record has no atomic coordinates; load a model first".into(); return; }
        if self.models.is_empty() { self.status = "No structure to save".into(); return; }
        match file_dialog::pick(file_dialog::Mode::Save, std::path::Path::new(&self.file_input), self.window.handle()) {
            Ok(Some(path)) => {
                self.path = path.to_string_lossy().into_owned();
                self.file_input=self.path.clone();
                self.focus = Focus::Neither;
                self.save();
            }
            Ok(None) => {}, // Canceling does not write anything or modify the path.
            Err(err) => self.status = format!("SAVE DIALOG ERROR: {err}"),
        }
    }
    fn save(&mut self){if self.record.is_some()&&self.models.is_empty(){self.status="Protein information records cannot be exported as molecular coordinates".into();return;}if self.models.is_empty(){self.status="No structure to save".into();return;}
        let path=self.path.to_ascii_lowercase();
        let all=[".sdf", ".pdb", ".ent", ".cif", ".mmcif", ".asnt", ".asn", ".pc3d", ".pc3"].iter().any(|ext|path.ends_with(ext));let models=if all{&self.models[..]}else{&self.models[self.index..=self.index]};
        self.status=match write_structures(&self.path,models){Ok(())=>format!("SAVED {} structure(s) to {}",models.len(),self.path),Err(e)=>format!("SAVE ERROR: {e}")};
    }
    fn on_click(&mut self,x:i32,y:i32){
        if self.has_3d_viewer() && self.viewer_toggle_rect().hit(x,y){
            self.toggle_viewer_expanded();return;
        }
        if self.viewer_expanded {
            self.viewer_focused=self.canvas_rect().hit(x,y);
            // The immersive layout intentionally has no top header, rail or footer.
            // Controls and docked details still receive normal hit-testing.
        } else {
            self.viewer_focused=self.has_3d_viewer() && self.canvas_rect().hit(x,y);
        }
        if self.help_open {
            self.help_open=false;
            self.focus=Focus::Neither;
            return;
        }
        if !self.viewer_expanded && self.ui().help_rect().hit(x,y){self.help_open=true;self.focus=Focus::Neither;return;}
        if !self.viewer_expanded {
            if let Some(tab)=Tab::ALL.iter().copied().find(|t|self.tab_rect(*t).hit(x,y)) {
                self.set_tab(tab);return;
            }
        }
        if !self.viewer_expanded && self.tab.is_view() && self.ui().details_rect().hit(x,y) {
            self.inspector_open=!self.inspector_open;
            self.status=if self.inspector_open{"DETAILS PANEL SHOWN (MAY REQUIRE A WIDER WINDOW)".into()}else{"DETAILS PANEL HIDDEN; CLICK DETAILS TO RESTORE".into()};
            return;
        }
        if self.tab==Tab::Lewis && self.formula_rect().hit(x,y){self.focus=Focus::Formula;return;}
        if !self.viewer_expanded && self.file_rect().hit(x,y){self.focus=Focus::File;return;}
        self.focus=Focus::Neither;
        if self.record.as_ref().is_some_and(|record|record.record_type.eq_ignore_ascii_case("protein")) && (self.tab==Tab::Record || (self.tab==Tab::Model3d && self.models.is_empty())) && self.fetch_rect().hit(x,y){
            self.start_protein_fetch();return;
        }
        if self.tab==Tab::Lewis && self.generate_rect().hit(x,y){self.recalc();return;}
        if !self.viewer_expanded && self.open_rect().hit(x,y){self.open_dialog();return;}
        if !self.viewer_expanded && self.save_rect().hit(x,y){self.save_dialog();return;}
        if self.tab==Tab::Cell {self.cell_handle_click(x,y);return;}
        if self.tab==Tab::Virus {self.virus_handle_click(x,y);return;}
        if self.tab==Tab::Settings {
            if self.settings_defaults_button().hit(x,y) {
                self.settings=ReconstructionSettings::default();
                self.settings.save();self.refresh_all_previews();
                self.status="RECONSTRUCTION SETTINGS RESTORED TO DEFAULTS".into();return;
            }
            for i in 0..5{if self.setting_row(i).hit(x,y){self.set_setting(i);break;}}
            return;
        }
        if self.tab==Tab::Files {
            if self.files_button().hit(x,y){self.open_dialog();return;}
            if self.files_show_all_button().hit(x,y){self.set_all_files_visible(true);return;}
            if self.files_hide_all_button().hit(x,y){self.set_all_files_visible(false);return;}
            let r=self.report_rect();
            let first=r.y+105;let row=((y-first)/50).max(0) as usize+self.file_scroll;
            let count=self.file_count();
            if y>=first && row<count && x>=r.x+20 && x<r.x+r.w-20 {
                if x<r.x+92{self.toggle_file(row);}
                else if x>=r.x+r.w-120{self.remove_file(row);}
                else{self.select_file(row);}
            }
            return;
        }
        if self.tab==Tab::Residues {
            let (inspect,focus)=self.residue_buttons();
            if inspect.hit(x,y){self.tab=Tab::Amino;return;}
            if focus.hit(x,y){if let Some(i)=self.selected_residue {self.focus_on_residue(i);}self.tab=Tab::Model3d;return;}
            let r=self.report_rect();
            let top=r.y+95;
            if x>=r.x+22 && x<r.x+(r.w*45/100) && y>=top {
                let row=(y-top)/27;
                if row>=0 {
                    let id=row as usize+self.residue_scroll;
                    if id<self.residues.len(){self.selected_residue=Some(id);self.selected_atom=None;self.show_electrons=false;}
                }
            }
            return;
        }
        if self.tab==Tab::Model3d {
            if self.focus_residue && self.selected_atom.is_some() {
                if self.electron_toggle_rect().hit(x,y) {
                    if !self.show_electrons && self.orbital_mask==0 {self.orbital_mask=15;}
                    self.show_electrons=!self.show_electrons;
                    self.status=if self.show_electrons {
                        "ATOMIC S/P/D/F CLOUDS ON | NEUTRAL ATOM REFERENCE ONLY".into()
                    } else {"ATOMIC ORBITAL CLOUDS OFF".into()};
                    return;
                }
                if let Some(index)=(0..4).find(|&i|self.orbital_filter_rect(i).hit(x,y)) {
                    self.orbital_mask^=1u8<<index;
                    self.show_electrons=self.orbital_mask!=0;
                    self.status="ORBITAL FILTER UPDATED | S/P/D/F ARE ATOMIC REFERENCES".into();
                    return;
                }
            }
            let controls=self.model_buttons();
            if let Some(i)=controls.iter().position(|r|r.hit(x,y)) {
                self.click_model_control(i);return;
            }
            let side=self.sidebar_rect();
            if side.hit(x,y) && self.current().is_some_and(view::is_protein) {
                let first=side.y+222;
                if y>=first && y<first+24*self.chains().len().min(4) as i32 {
                    let row=((y-first)/24) as usize;
                    if let Some(chain)=self.chains().get(row).cloned() {self.select_chain(chain);}
                }
                return;
            }
        }
        let slider=self.slider_rect();if !self.viewer_expanded && self.tab.has_variant() && Rect::new(slider.x,slider.y-13,slider.w,slider.h+26).hit(x,y){self.drag_variant=true;self.set_variant_from_x(x);return;}
        if self.tab==Tab::Model3d&&self.model_scene_rect().hit(x,y){
            self.drag_rotate=true;self.pick_origin=Some((x,y));
        }
        if self.tab==Tab::Amino&&self.report_rect().hit(x,y){self.drag_rotate=true;}
    }
    fn on_key(&mut self,key:Key){
        // MataWin's Key enum has no F11 variant. On Windows, F11 is polled
        // natively in run(), allowing an edge-triggered fullscreen toggle.
        // Ctrl+Shift+F is also supported via MataWin on every platform.
        if matches!(key, Key::Char('f') | Key::Char('F'))
            && (self.window.is_key_down(Key::LeftCtrl) || self.window.is_key_down(Key::RightCtrl))
            && (self.window.is_key_down(Key::LeftShift) || self.window.is_key_down(Key::RightShift))
        {
            if !self.f_held {
                self.f_held = true;
                self.toggle_app_fullscreen();
            }
            return;
        }
        if key==Key::Escape && self.viewer_expanded{
            self.toggle_viewer_expanded();return;
        }
        if key==Key::Escape && self.tab==Tab::Model3d && self.focus_residue {
            self.focus_residue=false;self.selected_atom=None;self.show_electrons=false;
            self.status="SHOWING WHOLE STRUCTURE | CLICK A RESIDUE TO FOCUS".into();
            return;
        }
        if matches!(key,Key::Char('f')|Key::Char('F')) && self.focus==Focus::Neither
            && self.has_3d_viewer() && (self.viewer_focused||self.viewer_expanded) {
            if !self.f_held{self.f_held=true;self.toggle_viewer_expanded();}
            return;
        }
        let ctrl = self.window.is_key_down(Key::LeftCtrl) || self.window.is_key_down(Key::RightCtrl);
        if ctrl {
            if let Key::Char(c) = key {
                match c.to_ascii_lowercase() {
                    'o' => { self.open_dialog(); return; }
                    's' => { self.save_dialog(); return; }
                    _ => {}
                }
            }
        }
        if key==Key::Escape{self.help_open=false;self.drag_rotate=false;self.drag_pan=false;self.drag_variant=false;self.drag_cell_slice=false;self.drag_virus_contour=false;self.focus=Focus::Neither;return;}
        if key==Key::Tab{self.viewer_expanded=false;self.viewer_focused=false;self.tab=match self.tab{Tab::Lewis=>Tab::Model3d,Tab::Model3d=>Tab::Vsepr,
            Tab::Vsepr=>Tab::Record,Tab::Record=>Tab::Files,Tab::Files=>Tab::Residues,
            Tab::Residues=>Tab::Amino,Tab::Amino=>Tab::Settings,Tab::Settings=>Tab::Cell,Tab::Cell=>Tab::Virus,Tab::Virus=>Tab::Lewis};return;}
        if self.focus==Focus::Neither && matches!(self.tab,Tab::Model3d|Tab::Cell|Tab::Virus) {
            match key {
                // Continuous W/A/S/D and Q/E input is sampled from the held-key
                // state every frame, not from KeyPressed (OS auto-repeat varies).
                Key::Char('p')|Key::Char('P') => {
                    self.render_preset=self.render_preset.next();
                    self.viewer.set_render_preset(self.render_preset);
                    self.virus_viewer.set_render_preset(self.render_preset);
                    self.cell_viewer.set_render_preset(self.render_preset);
                    self.status=format!("3D PERFORMANCE: {} | P CYCLES AUTO/ECONOMY/DETAIL | SOURCE DATA UNCHANGED",self.render_preset.label());
                    return;
                }
                Key::Char('g')|Key::Char('G') => {
                    self.toggle_gpu();return;
                }
                Key::Char('r')|Key::Char('R') => {self.reset_camera();return;}
                Key::Char('b')|Key::Char('B') if self.tab==Tab::Model3d => {self.click_model_control(1);return;}
                Key::Char('v')|Key::Char('V') if self.tab==Tab::Model3d => {self.click_model_control(2);return;}
                Key::Char('c')|Key::Char('C') if self.tab==Tab::Model3d => {self.click_model_control(5);return;}
                Key::Char('k')|Key::Char('K') if self.tab==Tab::Model3d => {self.click_model_control(7);return;}
                _=>{}
            }
        }
        if self.focus==Focus::Neither {
            if matches!(self.tab,Tab::Model3d|Tab::Amino) && !self.residues.is_empty(){
                if key==Key::Char('[') {self.selected_residue=Some(self.selected_residue.unwrap_or(0).saturating_sub(1));self.selected_atom=None;self.show_electrons=false;return;}
                if key==Key::Char(']') {self.selected_residue=Some((self.selected_residue.unwrap_or(0)+1).min(self.residues.len()-1));self.selected_atom=None;self.show_electrons=false;return;}
            }
            if key==Key::Left{self.index=self.index.saturating_sub(1);self.vsepr_scroll=0;self.cache_residues();}
            if key==Key::Right{self.index=(self.index+1).min(self.models.len().saturating_sub(1));self.vsepr_scroll=0;self.cache_residues();}
            if self.tab==Tab::Residues {
                if key==Key::Up{self.selected_residue=Some(self.selected_residue.unwrap_or(0).saturating_sub(1));}
                if key==Key::Down && !self.residues.is_empty(){self.selected_residue=Some((self.selected_residue.unwrap_or(0)+1).min(self.residues.len()-1));}
                if let Some(i)=self.selected_residue{let rows=((self.report_rect().h-145)/27).max(1) as usize;
                    if i<self.residue_scroll{self.residue_scroll=i;}
                    if i>=self.residue_scroll+rows{self.residue_scroll=i+1-rows;}
                }
            }
            if self.tab==Tab::Files && key==Key::Up{self.file_scroll=self.file_scroll.saturating_sub(1);}
            if self.tab==Tab::Files && key==Key::Down{self.file_scroll=self.file_scroll.saturating_add(1);}
            if matches!(self.tab,Tab::Vsepr|Tab::Record) && key==Key::Up{self.vsepr_scroll=self.vsepr_scroll.saturating_sub(1);}
            if matches!(self.tab,Tab::Vsepr|Tab::Record) && key==Key::Down{self.vsepr_scroll=self.vsepr_scroll.saturating_add(1);}
        }
        match key {
          Key::Enter=>if self.focus==Focus::File{
              // Manual path fallback: Shift+Enter saves directly, Enter opens.
              if self.window.is_key_down(Key::LeftShift)||self.window.is_key_down(Key::RightShift){
                  self.path=self.file_input.clone();self.save()
              }else{self.load()}
          }else if self.focus==Focus::Formula{self.recalc()}else if self.tab==Tab::Residues{self.tab=Tab::Amino},
          Key::Backspace=>match self.focus{Focus::Formula=>{self.formula.pop();},Focus::File=>{self.file_input.pop();},Focus::Neither=>{}},
          _=>{let shift=self.window.is_key_down(Key::LeftShift)||self.window.is_key_down(Key::RightShift);
              if let Some(c)=hit_key(key,shift){match self.focus {
                 Focus::Formula if self.formula.len()<80 =>self.formula.push(c),
                 Focus::File if self.file_input.len()<255 =>self.file_input.push(c),_=>{}
              }}
          }
        }
    }
    /// Sample physical key-down state, independent of key-repeat rate.
    fn camera_key_down(&self, letter: char) -> bool {
        self.window.is_key_down(Key::Char(letter))
            || self.window.is_key_down(Key::Char(letter.to_ascii_uppercase()))
    }

    fn advance_camera(&mut self, elapsed: f32) -> bool {
        if !matches!(self.tab,Tab::Model3d|Tab::Cell|Tab::Virus) || self.focus != Focus::Neither {
            // Don't continue drifting through another tab or text input.
            self.camera_motion.reset();
            return false;
        }
        let ctrl = self.window.is_key_down(Key::LeftCtrl) || self.window.is_key_down(Key::RightCtrl);
        let axes = if ctrl {
            (0.0, 0.0, 0.0)
        } else {
            (
                (self.camera_key_down('a') as i32 - self.camera_key_down('d') as i32) as f32,
                (self.camera_key_down('w') as i32 - self.camera_key_down('s') as i32) as f32,
                (self.camera_key_down('e') as i32 - self.camera_key_down('q') as i32) as f32,
            )
        };
        self.camera_motion.advance(elapsed, axes, &mut self.pan, &mut self.zoom)
    }

    fn button(&mut self,r:Rect,label:&str,primary:bool){let on=r.hit(self.mouse.0,self.mouse.1);
        draw::fill(&mut self.fb,r,if primary{if on{0xff3cdee5}else{0xff27b9c9}}else{if on{0xff2e4964}else{PANEL2}});
        draw::stroke(&mut self.fb,r,if primary{0xff56e7ef}else{STROKE});
        let preferred=font::width(label,2).unwrap_or(label.chars().count() as i32*12);
        let scale=if preferred>r.w-14{1}else{2};
        draw::centered(&mut self.fb,r.x+r.w/2,r.y+(r.h-7*scale)/2,label,scale,if primary{BG}else{WHITE});
    }
    fn field(&mut self,r:Rect,value:&str,focused:bool){draw::fill(&mut self.fb,r,0xff0b1829);
       draw::stroke(&mut self.fb,r,if focused{BLUE}else{STROKE});
       let s=font::tail(value,2,r.w-26);
       if !font::text(&mut self.fb,r.x+12,r.y+15,&s,2,WHITE,Some(r.w-26)) {
           draw::text_clip(&mut self.fb,r.x+12,r.y+15,&s,2,WHITE,((r.w-26)/12).max(1) as usize);
       }
    }
    fn panel(&mut self,r:Rect){draw::fill(&mut self.fb,r,PANEL);draw::stroke(&mut self.fb,r,STROKE)}
    fn draw_sidebar(&mut self,r:Rect){
       if self.tab==Tab::Model3d && self.focus_residue && self.selected_residue.is_some() {
           self.draw_residue_inspector(r);
           return;
       }
       self.panel(r);
       let Some(m)=self.models.get(self.index) else{
           if let Some(record)=self.record.as_ref(){
               draw::text(&mut self.fb,r.x+19,r.y+20,"PROTEIN RECORD",2,WHITE);
               draw::text_clip(&mut self.fb,r.x+19,r.y+64,&record.accession,2,BLUE,((r.w-38)/12).max(1) as usize);
               draw::text_clip(&mut self.fb,r.x+19,r.y+96,&record.title,1,GOLD,((r.w-38)/6).max(1) as usize);
               draw::text(&mut self.fb,r.x+19,r.y+129,&format!("{} RESIDUES",record.sequence.len()),1,MUTED);
               draw::text_clip(&mut self.fb,r.x+19,r.y+161,"NO ATOM COORDINATES",1,GOLD,((r.w-38)/6).max(1) as usize);
               draw::text_clip(&mut self.fb,r.x+19,r.y+185,"CLICK LOAD 3D IN RECORD TAB",1,MUTED,((r.w-38)/6).max(1) as usize);
           }
           return;
       };
       draw::text(&mut self.fb,r.x+19,r.y+20,"STRUCTURE",2,WHITE);
       let max=((r.w-38)/6).max(1) as usize;
       draw::text_clip(&mut self.fb,r.x+19,r.y+54,&m.name,1,GOLD,max);
       draw::text_clip(&mut self.fb,r.x+19,r.y+82,&format!("FORMULA  {}",m.formula()),1,MUTED,max);
       draw::text(&mut self.fb,r.x+19,r.y+106,&format!("ATOMS    {}",m.atoms.len()),1,MUTED);
       draw::text(&mut self.fb,r.x+19,r.y+125,&format!("BONDS    {}",m.bonds.len()),1,MUTED);
       draw::text(&mut self.fb,r.x+19,r.y+144,&format!("CHARGE   {}",m.total_charge()),1,MUTED);
       draw::fill(&mut self.fb,Rect::new(r.x+16,r.y+170,r.w-32,1),STROKE);
       if self.tab==Tab::Model3d{
          let mut visible_files=Vec::<(&str,&Molecule)>::new();
          if self.active_visible {visible_files.push((&self.path,m));}
          for entry in &self.other_files {if entry.visible {
              if let Some(m)=entry.models.get(entry.index){visible_files.push((&entry.path,m));}
          }}
          if visible_files.len()>1 && self.focused_chain.is_none() {
              draw::text(&mut self.fb,r.x+19,r.y+189,"VISIBLE FILES",2,WHITE);
              for (i,(path,model)) in visible_files.iter().take(((r.h-260)/38).max(1) as usize).enumerate(){
                  let yy=r.y+224+i as i32*38;
                  let chain=model.atoms.iter().filter_map(|a|a.site.as_ref())
                      .find(|s|s.atom_name.trim()=="CA").map(|s|s.chain_id.as_str()).unwrap_or("");
                  let color=view::chain_color(&format!("{i}:{chain}"));
                  draw::circle(&mut self.fb,r.x+28,yy+5,8,color);
                  let filename=std::path::Path::new(path).file_name().and_then(|f|f.to_str()).unwrap_or(path);
                  draw::text_clip(&mut self.fb,r.x+46,yy,&format!("{}",filename),1,WHITE,
                      ((r.w-61)/6).max(1) as usize);
                  draw::text(&mut self.fb,r.x+47,yy+18,&format!("{} ATOMS",model.atoms.len()),1,MUTED);
              }
              draw::text_clip(&mut self.fb,r.x+19,r.y+r.h-52,"LANES ARE NOT ALIGNED",1,GOLD,max);
              draw::text_clip(&mut self.fb,r.x+19,r.y+r.h-31,"FILES TAB: SELECT / TOGGLE",1,BLUE,max);
              return;
          }
          if view::is_protein(m) {
              draw::text(&mut self.fb,r.x+19,r.y+189,
                  if self.focused_chain.is_some() {"FOCUSED PROTEIN CHAIN"}else{"CHAINS: CLICK TO FOCUS"},1,WHITE);
              let mut chains=std::collections::BTreeMap::<String,usize>::new();
              for a in &m.atoms {if let Some(site)=a.site.as_ref() {
                  if site.atom_name.trim()=="CA" { *chains.entry(site.chain_id.clone()).or_default() += 1; }
              }}
              for (i,(chain,count)) in chains.iter().enumerate().take(4) {
                  let yy=r.y+222+i as i32*24;
                  let is_focused=self.focused_chain.as_ref()==Some(chain);
                  if is_focused {draw::fill(&mut self.fb,Rect::new(r.x+18,yy-3,r.w-36,23),0xff29506c);}
                  draw::circle(&mut self.fb,r.x+31,yy+6,8,view::chain_color(chain));
                  draw::text_clip(&mut self.fb,r.x+47,yy,&format!("{} {} ({} AA)",if is_focused{"*"}else{" "},chain,count),1,WHITE,((r.w-65)/6).max(1) as usize);
              }
          } else {
              draw::text(&mut self.fb,r.x+19,r.y+189,"ELEMENT COLORS",2,WHITE);
              let mut elements=std::collections::BTreeMap::new();for a in &m.atoms{*elements.entry(a.symbol.clone()).or_insert(0usize)+=1;}
              for (i,(sym,count)) in elements.iter().enumerate().take(((r.h-250)/29).max(0) as usize){let yy=r.y+222+i as i32*29;
                 draw::circle(&mut self.fb,r.x+31,yy+6,10,draw::element_color(sym));
                 draw::text_clip(&mut self.fb,r.x+54,yy,&format!("{} ({})",sym,count),2,WHITE,((r.w-65)/12).max(1) as usize);
              }
          }
       }else {
          let centers=if m.atoms.len()>750 {Vec::new()}else{vsepr_centers(&m)};
          draw::text(&mut self.fb,r.x+19,r.y+188,"VSEPR SHAPES",2,WHITE);
          draw::text_clip(&mut self.fb,r.x+19,r.y+211,
             &if m.atoms.len()>750 {"MACROMOLECULE VIEW".into()}else{format!("{} CENTRAL ATOM(S)",centers.len())},1,MUTED,max);
          if m.atoms.len()>750 {
            draw::text_clip(&mut self.fb,r.x+19,r.y+235,"PROTEIN: VSEPR VIEW LIMITED",1,MUTED,max);
          }else if centers.is_empty() {draw::text_clip(&mut self.fb,r.x+19,r.y+235,"NO CLASSIFIED CENTERS",1,MUTED,max);}
          for (i,c) in centers.iter().take(((r.h-256)/42).max(0) as usize).enumerate(){
             let yy=r.y+234+i as i32*42;
             draw::text_clip(&mut self.fb,r.x+19,yy,&format!("{}  {}",c.label(),c.axe_notation),1,BLUE,max);
             draw::text_clip(&mut self.fb,r.x+19,yy+17,c.molecular_geometry,1,WHITE,max);
          }
       }
       let yy=r.y+r.h-31;
       if yy>r.y+228 {
          draw::fill(&mut self.fb,Rect::new(r.x+16,yy-10,r.w-32,1),STROKE);
          draw::text_clip(&mut self.fb,r.x+19,yy,if self.tab==Tab::Model3d{"LEFT ROTATE | RIGHT PAN | B: BONDS"}else{"SEE VSEPR TAB FOR ALL"},1,BLUE,max);
       }
    }
    fn draw_protein_record(&mut self,r:Rect){
       self.panel(r);
       draw::text(&mut self.fb,r.x+22,r.y+19,"PUBCHEM INFORMATION RECORD",2,WHITE);
       if self.record.as_ref().is_some_and(|record|record.record_type.eq_ignore_ascii_case("protein")) {
           let label=if self.pending_model.is_some(){"LOADING..."}else if self.models.is_empty(){"LOAD 3D MODEL"}else{"RELOAD 3D"};
           self.button(self.fetch_rect(),label,false);
       }
       let Some(record)=self.record.as_ref() else{
           draw::text(&mut self.fb,r.x+22,r.y+79,"NO INFORMATION RECORD LOADED",2,MUTED);
           return;
       };
       let max=((r.w-44)/6).max(1) as usize;
       draw::text_clip(&mut self.fb,r.x+22,r.y+51,&record.title,2,GOLD,((r.w-44)/12).max(1) as usize);
       draw::text_clip(&mut self.fb,r.x+22,r.y+78,&format!("TYPE: {}    ACCESSION: {}",record.record_type,record.accession),1,BLUE,max);
       draw::text_clip(&mut self.fb,r.x+22,r.y+99,&record.url,1,MUTED,max);
       draw::text_clip(&mut self.fb,r.x+22,r.y+121,
           if self.models.is_empty() {"ANNOTATION FILE: LOAD PREDICTED 3D COORDINATES BELOW"}
           else {"3D MODEL LOADED: PREDICTED, NOT EXPERIMENTAL"},1,GOLD,max);

       for (i,(name,url)) in record.structure_links.iter().take(2).enumerate() {
           draw::text_clip(&mut self.fb,r.x+22,r.y+143+i as i32*19,&format!("{}: {}",name,url),1,BLUE,max);
       }
       draw::fill(&mut self.fb,Rect::new(r.x+18,r.y+189,r.w-36,1),STROKE);
       draw::text_clip(&mut self.fb,r.x+22,r.y+200,&format!("AMINO-ACID SEQUENCE ({} RESIDUES)",record.sequence.len()),1,WHITE,max);
       if record.sequence.is_empty() {
           draw::text_clip(&mut self.fb,r.x+22,r.y+235,"NO SEQUENCE ENCODED IN THIS RECORD",1,MUTED,max);
           return;
       }
       // PubChem protein sequences are ASCII amino-acid symbols.
       let line_width=((r.w-50)/8).clamp(10,120) as usize;
       let line_count=record.sequence.len().div_ceil(line_width);
       let visible=((r.h-265)/19).max(1) as usize;
       self.vsepr_scroll=self.vsepr_scroll.min(line_count.saturating_sub(visible));
       for row in 0..visible {
           let line=self.vsepr_scroll+row;
           let start=line*line_width;
           if start>=record.sequence.len(){break;}
           let end=(start+line_width).min(record.sequence.len());
           let sequence=&record.sequence[start..end];
           draw::text_clip(&mut self.fb,r.x+22,r.y+229+row as i32*19,sequence,1,WHITE,max);
       }
       draw::text_clip(&mut self.fb,r.x+22,r.y+r.h-23,
           &format!("SEQUENCE LINES {}-{} OF {} | MOUSE WHEEL / UP-DOWN TO SCROLL",
               self.vsepr_scroll+1,(self.vsepr_scroll+visible).min(line_count),line_count),1,BLUE,max);
    }
    fn draw_vsepr_report(&mut self,r:Rect){
       self.panel(r);
       draw::text(&mut self.fb,r.x+22,r.y+19,"VSEPR GEOMETRY BY CENTRAL ATOM",2,WHITE);
       let Some(m)=self.models.get(self.index) else{return;};
       if m.atoms.len()>750 {
           draw::text_clip(&mut self.fb,r.x+23,r.y+108,"PROTEIN / MACROMOLECULE: TOO MANY CENTERS FOR LEWIS VSEPR",1,GOLD,((r.w-45)/6).max(1) as usize);
           draw::text_clip(&mut self.fb,r.x+23,r.y+131,"VSEPR IS MAINLY INTENDED FOR SMALL MAIN-GROUP MOLECULES",1,MUTED,((r.w-45)/6).max(1) as usize);
           return;
       }
       let data=vsepr_centers(&m);
       draw::text_clip(&mut self.fb,r.x+23,r.y+49,&format!("{} CLASSIFIED CENTER(S)  |  ATOM NUMBERS MATCH LEWIS VIEW",data.len()),1,MUTED,((r.w-45)/6).max(1) as usize);
       if data.is_empty(){
           draw::text_clip(&mut self.fb,r.x+23,r.y+111,"NO SUPPORTED CENTRAL ATOMS IN THIS STRUCTURE",1,GOLD,((r.w-45)/6).max(1) as usize);
           draw::text_clip(&mut self.fb,r.x+23,r.y+133,"VSEPR REQUIRES AT LEAST TWO BONDED ATOMS PER CENTER",1,MUTED,((r.w-45)/6).max(1) as usize);
           return;
       }
       if r.h < 220 {
           draw::text_clip(&mut self.fb,r.x+23,r.y+92,"ENLARGE WINDOW TO READ VSEPR DETAILS",1,GOLD,((r.w-45)/6).max(1) as usize);
           return;
       }
       let available=r.h-165;
       let visible=(available/80).max(1) as usize;
       self.vsepr_scroll=self.vsepr_scroll.min(data.len().saturating_sub(visible));
       let max_text=((r.w-42)/6).max(1) as usize;
       for (row,center) in data.iter().skip(self.vsepr_scroll).take(visible).enumerate(){
          let yy=r.y+77+row as i32*80;
          draw::fill(&mut self.fb,Rect::new(r.x+17,yy,r.w-34,73),PANEL2);
          draw::fill(&mut self.fb,Rect::new(r.x+17,yy,3,73),BLUE);
          draw::text_clip(&mut self.fb,r.x+30,yy+9,&format!("ATOM {}  ({})",center.atom_index+1,center.symbol),2,BLUE,((r.w-66)/12).max(1) as usize);
          // Use one text row for the AXE notation and the actual molecular shape.
          draw::text_clip(&mut self.fb,r.x+235.min((r.w/3).max(80)),yy+10,&format!("{}   {}",center.axe_notation,center.molecular_geometry),2,WHITE,((r.w-262.min((r.w/3).max(107)))/12).max(1) as usize);
          draw::text_clip(&mut self.fb,r.x+30,yy+38,&format!("ELECTRON GEOMETRY: {}  |  BONDS: {}  LONE PAIRS: {}  DOMAINS: {}",center.electron_geometry,center.bonded_atoms,center.lone_pairs,center.electron_domains),1,MUTED,max_text);
          let implicit=if center.inferred_hydrogens>0 {format!("  |  {} H INFERRED FROM VALENCE",center.inferred_hydrogens)} else {String::new()};
          draw::text_clip(&mut self.fb,r.x+30,yy+55,&format!("TYPICAL ANGLES: {}{}",center.approximate_angles,implicit),1,GOLD,max_text);
       }
       let foot=r.y+r.h-55;
       if foot>r.y+100 {
           draw::fill(&mut self.fb,Rect::new(r.x+18,foot-9,r.w-36,1),STROKE);
           draw::text_clip(&mut self.fb,r.x+23,foot,&format!("SHOWING {}-{} OF {}  |  SCROLL OR UP/DOWN FOR MORE",self.vsepr_scroll+1,(self.vsepr_scroll+visible).min(data.len()),data.len()),1,BLUE,max_text);
           draw::text_clip(&mut self.fb,r.x+23,foot+18,"IDEALIZED VSEPR; IMPORTED STRUCTURES MAY OMIT HYDROGENS",1,MUTED,max_text);
       }
    }
    fn draw_files(&mut self,r:Rect){
        self.panel(r);
        draw::text(&mut self.fb,r.x+19,r.y+18,"LOADED STRUCTURES",2,WHITE);
        self.button(self.files_button(),"IMPORT MORE",false);
        self.button(self.files_show_all_button(),"SHOW ALL",false);
        self.button(self.files_hide_all_button(),"HIDE ALL",false);
        // Format the label before mutably borrowing the framebuffer.
        let file_count = self.file_count();
        let files_label = format!(
            "{} FILE(S) | CLICK SHOW/HIDE TO TOGGLE | CLICK NAME TO INSPECT",
            file_count,
        );
        draw::text_clip(&mut self.fb,r.x+19,r.y+53,
            &files_label,1,MUTED,((r.w-40)/6).max(1) as usize);
        draw::text_clip(&mut self.fb,r.x+19,r.y+74,
            "FILES ARE ARRANGED SIDE BY SIDE IN THE 3D VIEW (NOT COORDINATE-ALIGNED)",
            1,BLUE,((r.w-40)/6).max(1) as usize);
        let visible=((r.h-153)/50).max(1) as usize;
        let max_scroll=file_count.saturating_sub(visible);
        self.file_scroll=self.file_scroll.min(max_scroll);
        for slot in 0..visible {
            let row=self.file_scroll+slot;
            let file=if row==0 {
                if self.models.is_empty() && self.record.is_none(){continue;}
                Some((self.path.as_str(),self.active_visible,self.models.len(),self.current().map_or(0,|m|m.atoms.len()),true))
            }else {self.other_files.get(row-1).map(|f|(f.path.as_str(),f.visible,f.models.len(),
                f.models.get(f.index).map_or(0,|m|m.atoms.len()),false))};
            let Some((path,on,models,atoms,active))=file else{continue;};
            let yy=r.y+105+slot as i32*50;
            let rr=Rect::new(r.x+19,yy,r.w-38,44);
            draw::fill(&mut self.fb,rr,if active{0xff233b52}else{PANEL2});
            draw::stroke(&mut self.fb,rr,if active{BLUE}else{STROKE});
            let eye=Rect::new(rr.x+8,yy+9,59,24);
            draw::fill(&mut self.fb,eye,if on{0xff17656b}else{0xff293245});
            draw::centered(&mut self.fb,eye.x+eye.w/2,eye.y+7,if on{"HIDE"}else{"SHOW"},1,WHITE);
            let filename=std::path::Path::new(path).file_name().and_then(|p|p.to_str()).unwrap_or(path);
            draw::text_clip(&mut self.fb,rr.x+80,yy+6,&format!("{}{}",if active{"* "}else{""},filename),
                2,if active{BLUE}else{WHITE},((rr.w-215)/12).max(1) as usize);
            draw::text_clip(&mut self.fb,rr.x+81,yy+26,&format!("{} MODEL(S)   {} ATOMS   {}",models,atoms,
                if active{"ACTIVE"}else{"CLICK TO SELECT"}),1,MUTED,((rr.w-215)/6).max(1) as usize);
            let remove=Rect::new(rr.x+rr.w-98,yy+10,90,24);
            draw::fill(&mut self.fb,remove,0xff513044);
            draw::centered(&mut self.fb,remove.x+remove.w/2,remove.y+7,"REMOVE",1,WHITE);
        }
        let first = if file_count == 0 { 0 } else { self.file_scroll + 1 };
        let last = (self.file_scroll + visible).min(file_count);
        let footer = format!(
            "{}-{} / {} | MOUSE WHEEL TO SCROLL | CTRL+O TO ADD FILE",
            first, last, file_count,
        );
        draw::text_clip(&mut self.fb,r.x+20,r.y+r.h-30,
            &footer,1,BLUE,((r.w-44)/6).max(1) as usize);
    }
    fn draw_residues(&mut self,r:Rect){
        self.panel(r);
        draw::text(&mut self.fb,r.x+19,r.y+21,"PROTEIN RESIDUE EXPLORER",2,WHITE);
        let (inspect,focus)=self.residue_buttons();
        self.button(inspect,"CHEMICAL STRUCTURE",false);
        self.button(focus,"FOCUS IN 3D",true);
        if self.residues.is_empty() {
            draw::text_clip(&mut self.fb,r.x+22,r.y+102,
                "NO PDB/MMCIF RESIDUES IN THE SELECTED STRUCTURE",1,GOLD,((r.w-50)/6).max(1) as usize);
            return;
        }
        draw::text_clip(&mut self.fb,r.x+22,r.y+67,
            &format!("{} RESIDUES | SCROLL, CLICK, OR USE UP/DOWN TO SELECT",self.residues.len()),
            1,MUTED,((r.w-50)/6).max(1) as usize);
        let list_w=r.w*45/100;
        draw::fill(&mut self.fb,Rect::new(r.x+list_w,r.y+93,1,r.h-126),STROKE);
        let visible=((r.h-145)/27).max(1) as usize;
        self.residue_scroll=self.residue_scroll.min(self.residues.len().saturating_sub(visible));
        for row in 0..visible {
            let i=row+self.residue_scroll;let Some(res)=self.residues.get(i) else {break};
            let yy=r.y+95+row as i32*27;
            if self.selected_residue==Some(i) {
                draw::fill(&mut self.fb,Rect::new(r.x+19,yy,list_w-26,25),0xff28516b);
            }
            draw::text_clip(&mut self.fb,r.x+26,yy+5,
                &format!("{:>4}.  {}   {}  ({} ATOMS)",i+1,res.key.display(),res.one_letter,res.atoms.len()),
                1,if self.selected_residue==Some(i){BLUE}else{WHITE},((list_w-44)/6).max(1) as usize);
        }
        let Some(id)=self.selected_residue else{return;};
        let Some(res)=self.residues.get(id) else{return;};
        let left=r.x+list_w+24;let limit=((r.w-list_w-58)/6).max(1) as usize;
        draw::text_clip(&mut self.fb,left,r.y+106,&protein_inspect::full_name(&res.key.name).to_ascii_uppercase(),
            2,GOLD,((r.w-list_w-50)/12).max(1) as usize);
        draw::text_clip(&mut self.fb,left,r.y+141,&format!("CHAIN: {}    RESIDUE: {}{}",
            res.key.chain,res.key.number,res.key.insertion),1,WHITE,limit);
        draw::text_clip(&mut self.fb,left,r.y+164,&format!("CODE: {} / {}    OBSERVED ATOMS: {}",
            res.key.name,res.one_letter,res.atoms.len()),1,MUTED,limit);
        if let Some(model)=self.current(){
            let (phi,psi)=protein_inspect::phi_psi(model,&self.residues,id);
            let angle=|v:Option<f32>|v.map_or("N/A".to_string(),|d|format!("{d:.0} deg"));
            draw::text_clip(&mut self.fb,left,r.y+183,
                &format!("PHI: {}  |  PSI: {}",angle(phi),angle(psi)),1,WHITE,limit);
            draw::text_clip(&mut self.fb,left,r.y+201,
                protein_inspect::backbone_region(phi,psi),1,GOLD,limit);
        }
        let displayed_names=res.atoms.iter().filter_map(|&i|self.current().and_then(|m|m.atoms.get(i)))
            .filter_map(|a|a.site.as_ref().map(|s|s.atom_name.clone())).collect::<Vec<_>>();
        draw::text_clip(&mut self.fb,left,r.y+227,"OBSERVED ATOM NAMES",1,BLUE,limit);
        let chunk_size=((r.w-list_w-60)/70).max(3) as usize;
        for (i,group) in displayed_names.chunks(chunk_size).take(((r.h-354)/24).max(1) as usize).enumerate(){
            draw::text_clip(&mut self.fb,left,r.y+250+i as i32*22,&group.join("   "),1,WHITE,limit);
        }
        let chain_members=self.residues.iter().enumerate()
            .filter(|(_,x)|x.key.chain==res.key.chain && x.is_polymer).collect::<Vec<_>>();
        if r.h>=395 {if let Some(pos)=chain_members.iter().position(|(i,_)|*i==id){
            let begin=pos.saturating_sub(9);let end=(pos+10).min(chain_members.len());
            let mut sequence=String::new();
            for (offset,(_,item)) in chain_members[begin..end].iter().enumerate(){
                if begin+offset==pos {sequence.push('[');sequence.push(item.one_letter);sequence.push(']');}
                else {sequence.push(item.one_letter);}
            }
            draw::text_clip(&mut self.fb,left,r.y+r.h-91,"CHAIN SEQUENCE CONTEXT",1,BLUE,limit);
            draw::text_clip(&mut self.fb,left,r.y+r.h-72,&sequence,1,WHITE,limit);
        }}
        draw::text_clip(&mut self.fb,left,r.y+r.h-52,
            "NOTE: SOURCE MAY OMIT HYDROGENS OR PARTS OF A RESIDUE",1,GOLD,limit);
        draw::text_clip(&mut self.fb,left,r.y+r.h-29,
            "USE CHEMICAL STRUCTURE TO INSPECT BONDS AND ATOMS",1,MUTED,limit);
    }
    fn draw_amino(&mut self,r:Rect){
        self.panel(r);
        let Some(id)=self.selected_residue else {
            draw::text(&mut self.fb,r.x+22,r.y+40,"SELECT A RESIDUE IN RESIDUES TAB",2,GOLD);
            return;
        };
        let (structure,name)={
            let Some(res)=self.residues.get(id) else{return;};
            let Some(protein)=self.display_current() else{return;};
            let prepared_residues=protein_inspect::residues(protein);
            let selected=prepared_residues.iter().find(|r|r.key==res.key).unwrap_or(res);
            (protein_inspect::extract(protein,selected),
             format!("{}  |  {}",res.key.display(),protein_inspect::full_name(&res.key.name)))
        };
        draw::text_clip(&mut self.fb,r.x+19,r.y+17,&name,2,WHITE,((r.w-40)/12).max(1) as usize);
        draw::text_clip(&mut self.fb,r.x+19,r.y+51,
            &format!("DISPLAYED: {} ATOMS / {} BONDS  |  ELEMENTS: {}",structure.molecule.atoms.len(),
                structure.molecule.bonds.len(),structure.molecule.formula()),1,MUTED,((r.w-42)/6).max(1) as usize);
        let gap=16;let side=(r.w-gap*3)/2;
        let left=Rect::new(r.x+gap,r.y+91,side,r.h-184);
        let right=Rect::new(left.x+side+gap,left.y,side,left.h);
        draw::fill(&mut self.fb,left,0xff102034);
        draw::fill(&mut self.fb,right,0xff102034);
        draw::stroke(&mut self.fb,left,STROKE);draw::stroke(&mut self.fb,right,STROKE);
        draw::text(&mut self.fb,left.x+13,left.y+11,"CHEMICAL BONDS (2D)",1,BLUE);
        draw::text(&mut self.fb,right.x+13,right.y+11,"ATOMIC COORDINATES (3D)",1,BLUE);
        let mut schematic=structure.molecule.clone();
        schematic.from_file=false;
        for a in &mut schematic.atoms{a.pos=Vec3::default();}
        view::lewis(&mut self.fb,&schematic,Rect::new(left.x+9,left.y+34,left.w-18,left.h-46));
        self.viewer.draw(&mut self.fb,&structure.molecule,
            Rect::new(right.x+4,right.y+34,right.w-8,right.h-40),self.ax,self.ay,self.zoom);
        draw::text_clip(&mut self.fb,r.x+20,r.y+r.h-73,
            if structure.recovered_orders {"STANDARD RESIDUE TEMPLATE APPLIED TO SUPPLEMENT PDB/MMCIF BOND ORDERS"}
            else {"BONDS FROM INPUT STRUCTURE; UNKNOWN BOND ORDERS MAY BE INFERRED"},
            1,GOLD,((r.w-40)/6).max(1) as usize);
        draw::text_clip(&mut self.fb,r.x+20,r.y+r.h-50,
            if self.settings.highlight && !structure.molecule.estimated_atoms.is_empty() {"ORANGE ATOMS ARE APPROXIMATE MODELS, NOT EXPERIMENTAL COORDINATES"}
            else if structure.has_missing_hydrogens {"HYDROGENS MAY BE MISSING; OPEN SETTINGS TO ENABLE RECONSTRUCTION"}
            else {"OBSERVED ATOMS RETAIN THE COORDINATES FROM THE SOURCE FILE"},
            1,MUTED,((r.w-40)/6).max(1) as usize);
        draw::text_clip(&mut self.fb,r.x+20,r.y+r.h-27,
            "DRAG TO ROTATE 3D / WHEEL TO ZOOM / RESIDUES TAB TO SWITCH AMINO ACID",1,BLUE,((r.w-40)/6).max(1) as usize);
    }
    fn draw_settings(&mut self,r:Rect){
        self.panel(r);
        draw::text_clip(&mut self.fb,r.x+22,r.y+15,"STRUCTURE RECONSTRUCTION",2,WHITE,((r.w-210)/12).max(1) as usize);
        self.button(self.settings_defaults_button(),"RESTORE DEFAULTS",false);
        let switches=[
            ("Add missing hydrogens",self.settings.hydrogens),
            ("Infer missing bonds and bond orders",self.settings.bonds),
            ("Reconstruct incomplete side chains",self.settings.side_chains),
            ("Estimate missing backbone carbonyl oxygen",self.settings.backbone),
            ("Highlight estimated atoms",self.settings.highlight),
        ];
        for (idx,(name,enabled)) in switches.into_iter().enumerate(){
            let row=self.setting_row(idx);
            draw::fill(&mut self.fb,row,if enabled{0xff223d4a}else{0xff1a2b3e});
            draw::stroke(&mut self.fb,row,if enabled{BLUE}else{STROKE});
            draw::text_clip(&mut self.fb,row.x+12,row.y+11,name,1,WHITE,((row.w-110)/6).max(1) as usize);
            let switch=Rect::new(row.x+row.w-85,row.y+8,73,24);
            draw::fill(&mut self.fb,switch,if enabled{0xff275f52}else{0xff394454});
            draw::stroke(&mut self.fb,switch,if enabled{0xff59d5ac}else{STROKE});
            draw::centered(&mut self.fb,switch.x+switch.w/2,switch.y+6,if enabled{"ON"}else{"OFF"},1,WHITE);
        }
        let y=r.y+293;
        if let Some(preview)=&self.preview {
            let stats=preview.stats;
            draw::text_clip(&mut self.fb,r.x+24,y,
                &format!("MODELED: {} H / {} SIDE / {} BACKBONE | {} NEW BONDS, {} BOND ORDERS",stats.hydrogen_atoms,stats.side_chain_atoms,stats.backbone_atoms,stats.bonds_added,stats.orders_assigned),
                1,GOLD,((r.w-46)/6).max(1) as usize);
        }
        if r.h>335 {
            draw::text_clip(&mut self.fb,r.x+24,y+21,
                "ESTIMATES ARE APPROXIMATE, NOT ENERGY-MINIMIZED OR PH-ADJUSTED",1,MUTED,((r.w-46)/6).max(1) as usize);
            draw::text_clip(&mut self.fb,r.x+24,y+39,
                "SOURCE FILES AND ORIGINAL ATOM COORDINATES ARE ALWAYS PRESERVED",1,BLUE,((r.w-46)/6).max(1) as usize);
        }
    }
    fn draw_nav(&mut self) {
        let ui=self.ui();
        draw::fill(&mut self.fb,Rect::new(0,0,ui.rail,ui.height),0xff101d2b);
        draw::fill(&mut self.fb,Rect::new(ui.rail-1,0,1,ui.height),STROKE);
        draw::circle(&mut self.fb,ui.rail/2,40,21,BLUE);
        draw::centered(&mut self.fb,ui.rail/2,32,"M",2,BG);
        if ui.rail>100 {draw::centered(&mut self.fb,ui.rail/2,72,"MOLECULE STUDIO",1,WHITE);}
        if ui.rail>100 {
            for (y,label) in [(100,"EXPLORE"),(295,"ANALYZE"),(487,"WORKSPACE")] {
                draw::text(&mut self.fb,19,y,label,1,0xff69869e);
            }
        }else{
            for y in [108,302,494] {draw::fill(&mut self.fb,Rect::new(18,y,ui.rail-36,1),STROKE);}
        }
        for tab in Tab::ALL {
            let rect=ui.nav_rect(tab);
            let selected=self.tab==tab;
            let hovered=rect.hit(self.mouse.0,self.mouse.1);
            if selected {draw::fill(&mut self.fb,rect,0xff204357);
                draw::fill(&mut self.fb,Rect::new(rect.x,rect.y,3,rect.h),BLUE);
            }else if hovered{draw::fill(&mut self.fb,rect,0xff203044);}
            let text_color=if selected{WHITE}else if hovered{WHITE}else{MUTED};
            if ui.rail>100 {
                draw::fill(&mut self.fb,Rect::new(rect.x+13,rect.y+9,25,19),if selected{0xff2c8290}else{0xff243449});
                draw::centered(&mut self.fb,rect.x+26,rect.y+11,tab.short(),1,if selected{WHITE}else{MUTED});
                draw::text_clip(&mut self.fb,rect.x+47,rect.y+12,tab.nav_name(),1,text_color,21);
            }else {
                draw::centered(&mut self.fb,rect.x+rect.w/2,rect.y+11,tab.short(),1,text_color);
            }
        }
        let help=ui.help_rect();
        self.button(help,if ui.rail>100{"HELP / SHORTCUTS"}else{"?"},false);
        if ui.rail>100 {draw::text(&mut self.fb,20,ui.height-82,"OPEN FILES: CTRL+O",1,0xff69869e);}
    }
    fn draw_chrome(&mut self) {
        let ui=self.ui();
        // A smaller header saves vertical room for the interactive workspace.
        draw::fill(&mut self.fb,Rect::new(ui.rail,0,ui.width-ui.rail,79),0xff142337);
        draw::fill(&mut self.fb,Rect::new(ui.rail,78,ui.width-ui.rail,1),STROKE);
        draw::text_clip(&mut self.fb,ui.rail+24,17,self.tab.label(),2,WHITE,
            ((ui.width-ui.rail-400)/12).max(12) as usize);
        draw::text_clip(&mut self.fb,ui.rail+24,49,self.tab.description(),1,MUTED,
            ((ui.width-ui.rail-405)/6).max(12) as usize);
        if self.tab.is_view() {
            let label=if self.sidebar_rect().x<self.w(){"HIDE DETAILS"}else{"SHOW DETAILS"};
            self.button(ui.details_rect(),label,false);
        }
        self.button(self.open_rect(),"OPEN FILES",false);
        self.button(self.save_rect(),"SAVE AS",false);
        if self.tab==Tab::Lewis {
            draw::text(&mut self.fb,ui.rail+28,97,"CHEMICAL FORMULA",1,BLUE);
            let formula=self.formula.clone();self.field(self.formula_rect(),&formula,self.focus==Focus::Formula);
            self.button(self.generate_rect(),"BUILD",true);
        }
        if self.tab.has_variant() {
            let slider=self.slider_rect();
            let title_y=if self.tab==Tab::Lewis{169}else{104};
            draw::text(&mut self.fb,slider.x,title_y,"STRUCTURAL VARIANT",1,BLUE);
            draw::fill(&mut self.fb,Rect::new(slider.x,slider.y+4,slider.w,4),0xff32475e);
            if self.models.len()>1 {
                let count=self.models.len();
                let ticks=count.min(40);
                for i in 0..ticks {
                    let px=slider.x+(slider.w-1)*i as i32/(ticks-1) as i32;
                    draw::fill(&mut self.fb,Rect::new(px,slider.y+1,2,9),0xff627b96);
                }
            }
            let progress=if self.models.len()>1 {self.index as f32/(self.models.len()-1) as f32}else{0.0};
            draw::fill(&mut self.fb,Rect::new(slider.x,slider.y+4,(slider.w as f32*progress) as i32,4),BLUE);
            draw::circle(&mut self.fb,slider.x+(slider.w as f32*progress) as i32,slider.y+6,7,BLUE);
            let count=self.models.len();
            draw::text(&mut self.fb,slider.x+slider.w+20,slider.y-4,
                &format!("{} / {}",if count==0{0}else{self.index+1},count),1,WHITE);
            if self.tab!=Tab::Lewis {
                let selected_name=self.current().map(|m|m.name.clone()).unwrap_or_else(||"No structure selected".into());
                draw::text_clip(&mut self.fb,slider.x+205,title_y,&selected_name,1,GOLD,
                    ((slider.w-215)/6).max(1) as usize);
            }
        }
        if self.tab==Tab::Lewis {
            let title=self.current().map(|m|m.name.clone()).unwrap_or_else(||"No molecule loaded".into());
            draw::text_clip(&mut self.fb,ui.rail+30,202,&title,1,GOLD,
                ((ui.width-ui.rail-65)/6).max(1) as usize);
        }
    }
    fn draw_footer(&mut self) {
        let ui=self.ui();
        let y=ui.height-75;
        draw::fill(&mut self.fb,Rect::new(ui.rail,y,ui.width-ui.rail,75),0xff101d2b);
        draw::fill(&mut self.fb,Rect::new(ui.rail,y,ui.width-ui.rail,1),STROKE);
        let status_color=if self.status.contains("ERROR"){0xffff938b}else{0xffb9ccdc};
        draw::text_clip(&mut self.fb,ui.rail+22,y+10,&self.status,1,status_color,
            ((ui.width-ui.rail-48)/6).max(1) as usize);
        draw::text_clip(&mut self.fb,ui.rail+22,y+28,
            &format!("RENDERER: {} | P: {} | G: GPU",self.gpu_label,self.render_preset.label()),1,BLUE,
            ((ui.width-ui.rail-48)/6).max(1) as usize);
        draw::text(&mut self.fb,ui.rail+24,y+48,"PATH",1,BLUE);
        let path=self.file_input.clone();self.field(self.file_rect(),&path,self.focus==Focus::File);
    }
    fn draw_help(&mut self) {
        let ui=self.ui();
        let r=Rect::new(ui.content.x+18,ui.content.y+12,(ui.content.w-36).max(140),(ui.content.h-24).max(100));
        draw::fill(&mut self.fb,r,0xff192d41);draw::stroke(&mut self.fb,r,BLUE);
        draw::text(&mut self.fb,r.x+22,r.y+19,"GETTING STARTED",2,WHITE);
        draw::text(&mut self.fb,r.x+22,r.y+54,"Click anywhere to close",1,BLUE);
        let lines=[
            "1. OPEN FILES (Ctrl+O) to load molecules, proteins or microscopy",
            "2. Use the left sidebar to choose a workspace",
            "3. Rotate: left-drag    Pan: right-drag    Zoom: wheel",
            "4. Move: WASD    Zoom: Q/E    Reset view: R",
            "5. On 3D molecules: switch Backbone / All Atoms / Ribbon",
            "Performance: P cycles Auto / Economy / Detail; G toggles D3D11",
            "6. Protein residues: choose a residue, then Inspect amino acid",
            "7. Virus: link an active PDB/MMCIF to a viral protein",
            "8. Files: hide, show or select multiple loaded structures",
            "9. Settings: toggle reconstructed atoms and chemistry",
            "10. Formulas alone can represent multiple structural variants",
            "Ctrl+S: Save As       Tab: next workspace",
            "F11: app fullscreen       F: focused 3D viewer fullscreen",
            "Protein: click residue; click atom; toggle electron clouds in details",
        ];
        for (i,line) in lines.iter().enumerate() {
            let y=r.y+86+(i as i32)*26;
            if y+17>=r.y+r.h {break;}
            draw::text_clip(&mut self.fb,r.x+22,y,line,1,if i>=10{GOLD}else{WHITE},
                ((r.w-42)/6).max(1) as usize);
        }
        if r.h>415 {
            let font_label=format!("Text rendering: {}",self.font_info);
            draw::text_clip(&mut self.fb,r.x+22,r.y+r.h-30,&font_label,1,MUTED,
                ((r.w-42)/6).max(1) as usize);
        }
    }
    fn toggle_gpu(&mut self){
        #[cfg(target_os="windows")]
        {
            if self.gpu_enabled {
                self.gpu_enabled=false;
                self.gpu_label="CPU (GPU paused; G to resume)".to_owned();
            }else{
                if self.d3d11.is_none() {
                    let (w,h)=self.window.client_size();
                    match d3d11_studio::Layer::with_preference(self.window.handle(),w,h,
                        mata_3d::gpu_backend::GpuPreference::Auto){
                        Ok(layer)=>{self.gpu_label=format!("D3D11: {}",layer.name());self.d3d11=Some(layer);},
                        Err(error)=>{self.gpu_label=format!("CPU: GPU unavailable ({error})");return;},
                    }
                }
                self.gpu_enabled=true;
                self.gpu_label=format!("D3D11: {}",self.d3d11.as_ref().unwrap().name());
            }
            self.viewer.set_gpu_enabled(self.gpu_enabled);
        }
    }
    fn redraw(&mut self){
       let (w,h)=self.window.client_size();if w==0||h==0{return;}
       if self.fb.width!=w||self.fb.height!=h{self.fb.resize_and_clear(w,h,BG);}
       self.fb.clear(BG);
       self.viewer.set_gpu_enabled(self.gpu_enabled && self.tab==Tab::Model3d);
       self.cell_viewer.set_gpu_enabled(self.gpu_enabled && self.tab==Tab::Cell);
       self.virus_viewer.set_gpu_enabled(self.gpu_enabled && self.tab==Tab::Virus && !self.virus_experimental);
       #[cfg(target_os="windows")]
       if let Some(layer)=self.d3d11.as_mut(){layer.clear_frame();}
       if !self.viewer_expanded {self.draw_nav();self.draw_chrome();}
       if self.tab==Tab::Virus {
           self.draw_virus();
       }else if self.tab==Tab::Cell {
           self.draw_cells();
       }else if self.tab==Tab::Record {
           let report=self.report_rect();self.draw_protein_record(report);
       }else if self.tab==Tab::Vsepr {
           let report=self.report_rect();self.draw_vsepr_report(report);
       }else if self.tab==Tab::Files{
           let report=self.report_rect();self.draw_files(report);
       }else if self.tab==Tab::Residues{
           let report=self.report_rect();self.draw_residues(report);
       }else if self.tab==Tab::Amino{
           let report=self.report_rect();self.draw_amino(report);
       }else if self.tab==Tab::Settings{
           let report=self.report_rect();self.draw_settings(report);
       }else{
           let canvas=self.canvas_rect();self.panel(canvas);
           if self.tab==Tab::Lewis {
               if let Some(m)=self.preview.as_ref().map(|p|&p.molecule).or_else(||self.models.get(self.index)){
                   draw::text(&mut self.fb,canvas.x+17,canvas.y+9,"BLUE ATOM NUMBERS MATCH THE VSEPR REPORT",1,MUTED);
                   view::lewis(&mut self.fb,m,Rect::new(canvas.x+18,canvas.y+25,canvas.w-36,canvas.h-50));
               }else{
                   draw::centered(&mut self.fb,canvas.x+canvas.w/2,canvas.y+canvas.h/2,
                       "NO ACTIVE MOLECULE FOR LEWIS VIEW",1,MUTED);
               }
           }else{
               let mut visible=Vec::<&Molecule>::new();
               let mut active_index=None;
               if self.active_visible {
                   if let Some(m)=self.preview.as_ref().map(|p|&p.molecule).or_else(||self.models.get(self.index)){active_index=Some(visible.len());visible.push(m);}
               }
               for entry in &self.other_files {if entry.visible {
                   if let Some(other)=entry.preview.as_ref().map(|x|&x.molecule).or_else(||entry.models.get(entry.index)){visible.push(other);}
               }}
               if visible.is_empty(){
                   let label=if self.record.is_some(){"PROTEIN RECORD: NO COORDINATES EMBEDDED"}
                       else{"NO VISIBLE STRUCTURES - ENABLE ONE IN FILES TAB"};
                   draw::centered(&mut self.fb,canvas.x+canvas.w/2,canvas.y+canvas.h/2,label,1,MUTED);
               }else{
                   // The selection includes newly modeled atoms belonging
                   // to the same residue, while source indices stay unchanged.
                   let expanded_selection=self.selected_residue.map(|_|self.selected_residue_atoms());
                   let highlight=active_index.and_then(|file_index|expanded_selection.as_deref()
                       .map(|atoms|(file_index,atoms)));
                   // Chain isolation belongs to the active file, not to an
                   // unrelated visible file when the active one is hidden.
                   let chain_filter=if active_index.is_some() {self.focused_chain.as_deref()} else {None};
                   let scene_y=if self.tab==Tab::Model3d {self.model_toolbar_bottom()+9}else{canvas.y+1};
                   self.viewer.draw_scene(&mut self.fb,&visible,
                       Rect::new(canvas.x+1,scene_y,canvas.w-2,(canvas.y+canvas.h-scene_y-2).max(2)),
                       self.ax,self.ay,self.zoom,self.pan,self.detail_mode,
                       chain_filter,self.residue_colors,highlight,self.focus_residue && active_index.is_some(),
                       if self.focus_residue {self.selected_atom}else{None},
                       self.focus_residue && self.show_electrons,self.orbital_mask);
                   #[cfg(target_os="windows")]
                   if self.gpu_enabled {
                       if let (Some(gpu),Some((rect,_)))=(self.d3d11.as_mut(),self.viewer.gpu_scene()) {
                           gpu.capture_ui_underlay(&self.fb,rect);
                       }
                   }
               }
               if self.detail_mode==view::DetailMode::Ribbon {
                   let info="RIBBON: HELIX / STRAND / LOOP ESTIMATED FROM BACKBONE GEOMETRY";
                   let info_y=self.model_toolbar_bottom()+9;
                   draw::text_clip(&mut self.fb,canvas.x+14,info_y,info,1,MUTED,
                       ((canvas.w-28)/6).max(1) as usize);
               }
               if self.detail_mode==view::DetailMode::Ribbon && self.focused_chain.is_none() {
                   let bottom=canvas.y+canvas.h-54;
                   draw::fill(&mut self.fb,Rect::new(canvas.x+13,bottom,390,40),0xff172940);
                   draw::stroke(&mut self.fb,Rect::new(canvas.x+13,bottom,390,40),STROKE);
                   for (idx,(label,color)) in [("HELIX",0xffe979b0),("STRAND",0xffffd36c),("LOOP",0xff63c9dd)].iter().enumerate() {
                       let x=canvas.x+24+idx as i32*127;
                       draw::circle(&mut self.fb,x+4,bottom+20,5,*color);
                       draw::text(&mut self.fb,x+16,bottom+16,label,1,WHITE);
                   }
               }
               if let Some(chain)=self.focused_chain.as_ref().filter(|_|self.residue_colors && self.active_visible) {
                   // Chain colors identify residue *properties*, while atom
                   // sphere colors in all-atom mode still identify elements.
                   let bottom=canvas.y+canvas.h-112;
                   draw::fill(&mut self.fb,Rect::new(canvas.x+13,bottom,344,97),0xff172940);
                   draw::stroke(&mut self.fb,Rect::new(canvas.x+13,bottom,344,97),STROKE);
                   draw::text_clip(&mut self.fb,canvas.x+23,bottom+8,
                       &format!("CHAIN {} - AMINO ACID GROUPS",chain),1,WHITE,50);
                   for (i,(label,color)) in view::residue_legend().iter().enumerate(){
                       let xx=canvas.x+24+(i%2) as i32*170;
                       let yy=bottom+29+(i/2) as i32*16;
                       draw::circle(&mut self.fb,xx+5,yy+4,5,*color);
                       draw::text(&mut self.fb,xx+18,yy,label,1,WHITE);
                   }
               }
               if self.record.as_ref().is_some_and(|record|record.record_type.eq_ignore_ascii_case("protein")) && self.models.is_empty(){
                   let label=if self.pending_model.is_some(){"LOADING..."}else{"LOAD 3D MODEL"};
                   self.button(self.fetch_rect(),label,false);
               }
           }
           if self.tab==Tab::Model3d {self.draw_model_toolbar();}
           let sidebar=self.sidebar_rect();if sidebar.x<self.w(){self.draw_sidebar(sidebar);}
       }
       if self.has_3d_viewer(){self.draw_viewer_toggle();}
       if !self.viewer_expanded {self.draw_footer();}
       if self.help_open {self.draw_help();}
       #[cfg(target_os="windows")]
       if self.gpu_enabled && matches!(self.tab,Tab::Model3d|Tab::Cell|Tab::Virus) {
           let scene=match self.tab {
               Tab::Model3d=>self.viewer.gpu_scene(),
               Tab::Cell=>self.cell_viewer.gpu_scene(),
               Tab::Virus if !self.virus_experimental=>self.virus_viewer.gpu_scene(),
               _=>None,
           };
           if self.d3d11.as_ref().is_some_and(|layer|layer.has_capture()) {
           if let (Some(layer),Some((_,triangles)))=(self.d3d11.as_mut(),scene) {
               match layer.present(&self.fb,triangles) {
                   Ok(())=>return,
                   Err(error)=>{
                       eprintln!("[Mata Molecule] D3D11 presentation failed: {error}; reverting to CPU");
                       self.d3d11=None;
                       self.gpu_enabled=false;
                       self.viewer.set_gpu_enabled(false);
                       self.cell_viewer.set_gpu_enabled(false);
                       self.virus_viewer.set_gpu_enabled(false);
                       self.gpu_label=format!("CPU (D3D11 failed: {error})");
                       // Rebuild the scene using the CPU rasterizer immediately,
                       // so the first fallback frame never displays empty atoms.
                       self.redraw();
                       return;
                   }
               }
           }
           }
       }
       self.window.present_exact(&self.fb);
    }
    fn run(mut self){
        let mut dirty=true;
        let mut last_frame=Instant::now();
        let render_stats=std::env::var_os("MATA_RENDER_STATS").is_some();
        let (mut profile_count,mut profile_total_ms)=(0u32,0.0f32);
        loop {
           self.window.poll_events_into(&mut self.events);
           let events=self.events.clone();
           let mut quit=false;
           for event in events {match event{
             WindowEvent::Close=>quit=true,
             WindowEvent::Resized(_,_)|WindowEvent::Maximized(_,_)|WindowEvent::Restored(_,_)=>dirty=true,
             WindowEvent::CursorMoved(x,y)=>{let old=self.mouse;self.mouse=(x,y);
                let old_rotation=(self.ax,self.ay);
                let old_pan=self.pan;
                if self.drag_rotate{self.ay+=(x-old.0) as f32*0.012;self.ax=((self.ax+(y-old.1) as f32*0.012)).clamp(-PI*0.5,PI*0.5);}
                if self.drag_pan {
                    let size=if matches!(self.tab,Tab::Model3d|Tab::Cell|Tab::Virus){self.canvas_rect().h}else{self.report_rect().h}.max(1) as f32;
                    self.pan.0+=(x-old.0) as f32/size;
                    self.pan.1-=(y-old.1) as f32/size;
                }
                if self.drag_variant{self.set_variant_from_x(x);}
                if self.drag_cell_slice {
                    let s=self.cell_slider();
                    let t=((x-s.x) as f32/s.w.max(1) as f32).clamp(0.0,1.0);
                    if self.cell_mode==VolumeMode::Surface{self.cell_threshold=t.clamp(0.001,0.999)}else{self.cell_slice=t}
                }
                if self.drag_virus_contour {
                    let s=self.virus_contour_slider();
                    self.virus_threshold=((x-s.x) as f32/s.w.max(1) as f32).clamp(0.001,0.999);
                }
                // Hover highlights still refresh if the cursor is over a
                // toolbar, but the expensive 3D scene is not invalidated by
                // an otherwise idle pointer move (full UI redraw currently
                // reuses the same immediate-mode presentation pipeline).
                let scene_hover=self.tab==Tab::Model3d
                    && self.model_scene_rect().hit(x,y)
                    && self.model_scene_rect().hit(old.0,old.1);
                dirty|=self.drag_variant||self.drag_cell_slice||self.drag_virus_contour
                    ||old_rotation!=(self.ax,self.ay)||old_pan!=self.pan||!scene_hover;
             },
             WindowEvent::KeyPressed(Key::MouseLeft)=>{
                let shift=self.window.is_key_down(Key::LeftShift)||self.window.is_key_down(Key::RightShift);
                if shift && self.has_3d_viewer() && self.canvas_rect().hit(self.mouse.0,self.mouse.1)
                    && !self.viewer_toggle_rect().hit(self.mouse.0,self.mouse.1) {
                    self.drag_pan=true;self.viewer_focused=true;self.focus=Focus::Neither;
                }else{self.on_click(self.mouse.0,self.mouse.1);}
                dirty=true;
             },
             WindowEvent::KeyReleased(Key::MouseLeft)=>{
                if let Some((sx,sy))=self.pick_origin.take() {
                    let (x,y)=self.mouse;
                    if (x-sx).abs()<=5 && (y-sy).abs()<=5 && self.model_scene_rect().hit(x,y) {
                        self.pick_model(x,y);
                    }
                }
                self.drag_rotate=false;self.drag_pan=false;self.drag_variant=false;
                self.drag_cell_slice=false;self.drag_virus_contour=false;dirty=true;
             },
             WindowEvent::KeyPressed(Key::MouseRight)|WindowEvent::KeyPressed(Key::MouseMiddle)=>{
                if self.has_3d_viewer() && self.canvas_rect().hit(self.mouse.0,self.mouse.1) {
                    self.drag_pan=true;self.viewer_focused=true;self.focus=Focus::Neither;
                }
             },
             WindowEvent::KeyReleased(Key::MouseRight)|WindowEvent::KeyReleased(Key::MouseMiddle)=>{
                self.drag_pan=false;
             },
             WindowEvent::Scroll(_,dy)=>{
                if self.tab==Tab::Model3d && self.focus_residue && self.selected_atom.is_some()
                    && self.sidebar_rect().hit(self.mouse.0,self.mouse.1) {
                    if dy>0 {self.electron_neighbor_scroll=self.electron_neighbor_scroll.saturating_sub(1);}
                    else if dy<0 {self.electron_neighbor_scroll=self.electron_neighbor_scroll.saturating_add(1);}
                    dirty=true;continue;
                }
                if self.tab==Tab::Cell && self.sidebar_rect().hit(self.mouse.0,self.mouse.1) {
                    if dy>0{self.cell_scroll=self.cell_scroll.saturating_sub(1);}else if dy<0{self.cell_scroll=self.cell_scroll.saturating_add(1);}dirty=true;continue;
                }
                if matches!(self.tab,Tab::Model3d|Tab::Amino|Tab::Cell|Tab::Virus){
                    // Scroll both directions without a fixed zoom ceiling or floor.
                    let factor=if dy>0 {1.09} else if dy<0 {1.0/1.09} else {1.0};
                    dirty |= camera_motion::multiply_zoom(&mut self.zoom,factor);
                }
                else if matches!(self.tab,Tab::Vsepr|Tab::Record|Tab::Residues|Tab::Files){
                    if self.tab==Tab::Files {if dy.signum()>0{self.file_scroll=self.file_scroll.saturating_sub(1)}else{self.file_scroll+=1;}dirty=true;continue;}
                    if self.tab==Tab::Residues {if dy.signum()>0{self.residue_scroll=self.residue_scroll.saturating_sub(2)}else{self.residue_scroll+=2;}dirty=true;continue;}
                    let steps=1usize;
                    if dy.signum()>0{self.vsepr_scroll=self.vsepr_scroll.saturating_sub(steps);}
                    else if dy.signum()<0{self.vsepr_scroll=self.vsepr_scroll.saturating_add(steps);}
                    dirty=true;
                }
             },
             WindowEvent::KeyReleased(Key::Char('f')|Key::Char('F'))=>{self.f_held=false;},
             WindowEvent::KeyPressed(k)=>{self.on_key(k);dirty=true;},
             _=>{}
           }}
           if quit{break;}
           // Poll the native F11 key only when this window owns foreground
           // focus. Track the key's held state so a long press toggles once.
           let f11_down = window_fullscreen::native_f11_down(self.window.handle());
           if f11_down && !self.f11_held {
               self.f11_held = true;
               self.toggle_app_fullscreen();
               dirty = true;
           } else if !f11_down {
               self.f11_held = false;
           }
           if self.poll_protein_fetch(){dirty=true;}
           let now=Instant::now();
           let elapsed=now.duration_since(last_frame).as_secs_f32();
           last_frame=now;
           if self.advance_camera(elapsed){dirty=true;}
           if dirty {
               self.viewer.interactive=self.drag_rotate||self.drag_pan;
               let started=Instant::now();
               self.redraw();
               if matches!(self.tab,Tab::Model3d|Tab::Amino) {
                   let ms=started.elapsed().as_secs_f32()*1000.0;
                   self.viewer.observe_frame_ms(ms);
                   if render_stats {
                       profile_count+=1;profile_total_ms+=ms;
                       if profile_count>=60 {
                           eprintln!("Mata 3D: avg redraw {:.1} ms, geometry quality {:.2}, {:.0} redraws/s",
                               profile_total_ms/profile_count as f32,self.viewer.lod_quality(),
                               1000.0*profile_count as f32/profile_total_ms.max(0.001));
                           profile_count=0;profile_total_ms=0.0;
                       }
                   }
               }
               dirty=false;
           }
           std::thread::sleep(Duration::from_millis(12));
        }
    }
}
use std::f32::consts::PI;
fn main(){
    // Shader source is exportable without a GPU, driver or vendor SDK. It
    // becomes a hardware path only when a renderer compiles and dispatches it.
    let mut launch_args=std::env::args().skip(1);
    if let Some(flag)=launch_args.next() {
        if flag=="--export-gpu-shaders" {
            let directory=launch_args.next().unwrap_or_else(||"gpu-shaders".into());
            match gpu_shaders::export_to(std::path::Path::new(&directory)) {
                Ok(())=>println!("Shader source written to {directory}"),
                Err(err)=>eprintln!("GPU shader export failed: {err}"),
            }
            return;
        }
    }
    let mut studio=Studio::new();
    let mut args=std::env::args().skip(1);
    let mut requested_font=None::<String>;
    while let Some(arg)=args.next(){match arg.as_str(){
        "--open"=>if let Some(path)=args.next(){studio.open_path(path)},
        "--formula"=>if let Some(formula)=args.next(){studio.formula=formula;studio.recalc()},
        "--font"=>requested_font=args.next(),
        _=>eprintln!("Unknown argument: {arg} (use --open PATH, --formula FORMULA, or --font PATH)"),
    }}
    studio.font_info=font::initialize(requested_font.as_deref().map(std::path::Path::new));
    eprintln!("Font: {}",studio.font_info);
    studio.run();
}

#[cfg(test)]
mod physical_button_tests {
    use super::*;

    #[test]
    fn all_nine_model_buttons_are_clickable_and_non_overlapping() {
        for width in [285, 420, 650, 814, 1200] {
            let area=Rect::new(28,303,width,420);
            let controls=model_toolbar_layout(area);
            for (i,button) in controls.iter().enumerate() {
                assert!(button.w>0 && button.h==32);
                assert!(button.x>=area.x+12);
                assert!(button.x+button.w<=area.x+area.w-12);
                assert!(button.y>=area.y+12);
                assert!(button.y+button.h<=area.y+area.h);
                assert!(button.hit(button.x+button.w/2,button.y+16));
                for other in &controls[..i] {
                    assert!(button.x>=other.x+other.w || other.x>=button.x+button.w
                         || button.y>=other.y+other.h || other.y>=button.y+button.h);
                }
            }
        }
    }

    #[test]
    fn model_view_mode_buttons_are_a_distinct_first_group() {
        let controls=model_toolbar_layout(Rect::new(28,303,814,340));
        assert_eq!(controls[0].y, controls[1].y);
        assert_eq!(controls[1].y, controls[2].y);
        assert!(controls[5].y>controls[0].y);
    }
}

#[cfg(test)]
mod virus_toolbar_tests {
    use super::*;
    #[test]
    fn all_virus_controls_have_distinct_clickable_bounds() {
        for width in [280,420,814,1120] {
            let area=Rect::new(28,303,width,340);
            let widths=[120,132,96,122,120,116,115,98];
            let (mut x,mut y)=(area.x+12,area.y+10);
            let mut rects=Vec::new();
            for desired in widths {
                let w=desired.min((area.w-24).max(1));
                if x>area.x+12 && x+w>area.x+area.w-12 {x=area.x+12;y+=35;}
                let r=Rect::new(x,y,w,29);
                assert!(r.x>=area.x+12 && r.x+r.w<=area.x+area.w-12);
                for prior in &rects {let other:&Rect=prior;assert!(r.x>=other.x+other.w || other.x>=r.x+r.w || r.y>=other.y+other.h || other.y>=r.y+r.h);}
                rects.push(r);x+=w+6;
            }
            assert_eq!(rects.len(),8);
            assert!(rects.last().unwrap().y+29<area.y+area.h);
        }
    }
}

#[cfg(test)]
mod redesigned_shell_tests {
    use super::*;

    #[test]
    fn all_navigation_destinations_have_unique_click_targets() {
        let shell=UiShell::new(1180,760,Tab::Model3d);
        for (index,tab) in Tab::ALL.iter().enumerate() {
            let button=shell.nav_rect(*tab);
            assert!(button.w>0 && button.h>=32);
            assert!(button.x>=0 && button.x+button.w<=shell.rail);
            for other in &Tab::ALL[..index] {
                let prev=shell.nav_rect(*other);
                assert!(!prev.hit(button.x+button.w/2,button.y+button.h/2));
            }
        }
    }

    #[test]
    fn inspectors_can_be_hidden_without_removing_the_view() {
        for width in [550,800,1180,1600] {
            let shell=UiShell::new(width,800,Tab::Model3d);
            let expanded=shell.canvas_rect(false);
            let collapsed=shell.canvas_rect(true);
            assert!(expanded.w>collapsed.w);
            assert!(collapsed.w>120);
            let side=shell.inspector_rect(true);
            assert!(collapsed.x+collapsed.w+12==side.x);
            assert!(side.x+side.w<=width);
        }
    }

    #[test]
    fn context_controls_only_use_the_space_they_need() {
        let lewis=UiShell::new(1180,760,Tab::Lewis);
        let model=UiShell::new(1180,760,Tab::Model3d);
        let virus=UiShell::new(1180,760,Tab::Virus);
        assert!(lewis.content.y>model.content.y);
        assert!(model.content.y>virus.content.y);
        assert!(lewis.formula_rect().x+lewis.formula_rect().w<lewis.build_rect().x);
        assert!(virus.content.h>model.content.h);
    }
}

#[cfg(test)]
mod focused_viewer_tests {
    use super::*;

    #[test]
    fn immersive_layout_uses_the_window_and_docks_information_to_right() {
        for (w,h) in [(550,450),(1180,760),(1920,1080),(3840,2160)] {
            let ui=UiShell::immersive(w,h);
            let view=ui.canvas_rect(true);
            let dock=ui.inspector_rect(true);
            assert_eq!(ui.content.x,8);
            assert_eq!(ui.content.y,8);
            assert_eq!(view.x,8);
            assert_eq!(view.y,8);
            assert_eq!(view.h,h-16);
            assert_eq!(dock.h,view.h);
            assert!(view.w>=120);
            assert_eq!(view.x+view.w+12,dock.x);
            assert_eq!(dock.x+dock.w,w-8);
        }
    }
    #[test]
    fn viewer_fullscreen_button_stays_above_model_toolbar() {
        let ui=UiShell::immersive(1180,760);
        let c=ui.canvas_rect(true);
        let button=Rect::new(c.x+c.w-144,c.y+7,134,30);
        let toolbar=model_toolbar_layout(Rect::new(c.x,c.y+36,c.w,c.h-36));
        assert!(toolbar.iter().all(|item|item.y>=button.y+button.h));
    }
}
