//! Five-mission story campaign with a persistent, optional fifteen-record journal.
use std::{fs,io,path::PathBuf};
use crate::{levels::MISSIONS,lore,world::World};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Screen { Title, Prologue, Briefing, Playing, Paused, Journal, StoryIndex, Story, Debrief, GameOver, Ending }

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Progress {
    level:usize,
    unlocked:usize,
    score:u32,
    health:i32,
    ammo:i32,
    kills:u32,
    best_score:u32,
    finished:bool,
    lore_mask:u32,
}
impl Progress {
    fn fresh()->Self{Self{level:0,unlocked:0,score:0,health:100,ammo:36,kills:0,best_score:0,finished:false,lore_mask:0}}
    fn encode(self)->String{format!("MATA_CAMPAIGN_V2\n{} {} {} {} {} {} {} {} {}\n",
        self.level,self.unlocked,self.score,self.health,self.ammo,self.kills,self.best_score,u8::from(self.finished),self.lore_mask)}
    fn decode(source:&str)->Option<Self>{
        let mut lines=source.lines();
        let version=lines.next()?;
        if !matches!(version,"MATA_CAMPAIGN_V1"|"MATA_CAMPAIGN_V2"){return None}
        let values:Vec<_>=lines.next()?.split_whitespace().collect();
        if values.len()!=if version=="MATA_CAMPAIGN_V2"{9}else{8}{return None}
        let mut p=Self{
            level:values[0].parse().ok()?,unlocked:values[1].parse().ok()?,
            score:values[2].parse().ok()?,health:values[3].parse().ok()?,
            ammo:values[4].parse().ok()?,kills:values[5].parse().ok()?,
            best_score:values[6].parse().ok()?,finished:match values[7]{"0"=>false,"1"=>true,_=>return None},
            lore_mask:if version=="MATA_CAMPAIGN_V2"{values[8].parse().ok()?}else{0},
        };
        // A completed old four-act save unlocks the NEW fifth act, not a
        // premature victory. Earlier checkpoints retain their mission.
        if version=="MATA_CAMPAIGN_V1" {
            if p.level>3 || p.unlocked>3 || p.level>p.unlocked{return None}
            if p.finished {
                p.level=4;p.unlocked=4;p.finished=false;
                p.health=p.health.max(75);p.ammo=p.ammo.max(40);
            }
        }
        let all_lore=(1u32<<lore::RECORDS.len())-1;
        (p.level<MISSIONS.len() && p.unlocked<MISSIONS.len()
            && p.level<=p.unlocked && (1..=100).contains(&p.health)
            && (0..=99).contains(&p.ammo) && p.lore_mask & !all_lore == 0).then_some(p)
    }
}
fn save_path()->PathBuf {
    #[cfg(target_os="windows")]
    let base=std::env::var_os("LOCALAPPDATA").or_else(||std::env::var_os("APPDATA")).map(PathBuf::from);
    #[cfg(not(target_os="windows"))]
    let base=std::env::var_os("XDG_STATE_HOME").map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|h|PathBuf::from(h).join(".local/state")));
    base.unwrap_or_else(||std::env::current_dir().unwrap_or_else(|_|PathBuf::from(".")))
        .join("mata-raycast").join("campaign.save")
}

pub struct Campaign {
    pub world:World,
    pub screen:Screen,
    pub completed_kills:u32,
    pub level_score:u32,
    pub save_status:String,
    pub last_level_time:f32,
    pub best_score:u32,
    pub has_save:bool,
    pub finished:bool,
    pub lore_mask:u32,
    pub journal_entry:usize,
    pub journal_page:usize,
    pub story_chapter:usize,
    pub story_page:usize,
    pub story_page_count:usize,
    pub story_selection:usize,
    story_return:Screen,
    library_return:Screen,
    story_auto:bool,
    checkpoint:Progress,
    save_file:PathBuf,
}
impl Campaign {
    pub fn new()->Self {
        let save_file=save_path();
        let previous=fs::read_to_string(&save_file).ok().and_then(|data|Progress::decode(&data));
        let checkpoint=previous.unwrap_or_else(Progress::fresh);
        Self{world:World::new(),screen:Screen::Title,completed_kills:0,
            level_score:0,save_status:String::new(),last_level_time:0.0,
            best_score:checkpoint.best_score,has_save:previous.is_some(),finished:checkpoint.finished,
            lore_mask:checkpoint.lore_mask,journal_entry:0,journal_page:0,
            story_chapter:0,story_page:0,story_page_count:1,story_selection:0,
            story_return:Screen::Title,library_return:Screen::Title,story_auto:false,
            checkpoint,save_file}
    }
    fn write_save(&mut self){
        let result=(||->io::Result<()> {
            if let Some(dir)=self.save_file.parent(){fs::create_dir_all(dir)?;}
            let temporary=self.save_file.with_extension("save.tmp");
            fs::write(&temporary,self.checkpoint.encode())?;
            if fs::rename(&temporary,&self.save_file).is_err(){
                let _=fs::remove_file(&self.save_file);
                fs::rename(&temporary,&self.save_file)?;
            }
            Ok(())
        })();
        match result {
            Ok(())=>{self.has_save=true;self.save_status="CAMPAIGN CHECKPOINT SAVED".into();},
            Err(error)=>{self.save_status=format!("SAVE FAILED: {error}");},
        }
    }
    pub fn new_campaign(&mut self){
        let previous_best=self.best_score;
        self.checkpoint=Progress::fresh();self.checkpoint.best_score=previous_best;
        self.completed_kills=0;self.finished=false;self.lore_mask=0;
        self.begin_checkpoint();self.write_save();
        self.open_story_auto(0,Screen::Briefing);
    }
    pub fn continue_campaign(&mut self){
        if !self.has_save{self.new_campaign();return;}
        self.begin_checkpoint();
    }
    fn begin_checkpoint(&mut self){
        let p=self.checkpoint;
        self.world=World::for_level(p.level,p.health,p.ammo,p.score);
        self.completed_kills=p.kills;self.best_score=p.best_score;self.lore_mask=p.lore_mask;
        self.level_score=0;self.last_level_time=0.0;
        self.screen=Screen::Briefing;
    }
    pub fn retry(&mut self){self.begin_checkpoint();}
    pub fn return_to_title(&mut self){self.screen=Screen::Title;}

    /// Acts unlock after completion so reading never reveals a later mission.
    /// The supplementary recovered scenes unlock after Act III. The full
    /// evidence index and epilogue unlock only when the campaign is finished.
    pub fn story_unlocked(&self,chapter:usize)->bool {
        match chapter {
            0=>true,
            1..=5=>self.finished || self.checkpoint.unlocked >= chapter,
            6|8=>self.finished,
            7=>self.finished || self.checkpoint.unlocked >= 3,
            _=>false,
        }
    }
    pub fn open_library(&mut self){
        if self.screen==Screen::StoryIndex{return}
        self.library_return=self.screen;
        self.story_selection=self.story_chapter;
        self.screen=Screen::StoryIndex;
    }
    pub fn close_library(&mut self){self.screen=self.library_return;}
    pub fn story_select(&mut self,delta:i32){
        self.story_selection=(self.story_selection as i32+delta).clamp(0,(crate::story::CHAPTER_TITLES.len()-1) as i32) as usize;
    }
    pub fn story_open_selection(&mut self){
        if !self.story_unlocked(self.story_selection){return}
        self.open_story(self.story_selection,Screen::StoryIndex,false);
    }
    fn open_story(&mut self,chapter:usize,return_to:Screen,auto:bool){
        self.story_chapter=chapter;self.story_selection=chapter;
        self.story_page=0;self.story_page_count=1;
        self.story_return=return_to;self.story_auto=auto;
        self.screen=Screen::Story;
    }
    fn open_story_auto(&mut self,chapter:usize,return_to:Screen){
        self.open_story(chapter,return_to,true);
    }
    pub fn story_change_chapter(&mut self,delta:i32){
        let mut ix=self.story_chapter as i32+delta.signum();
        while ix>=0 && ix<crate::story::CHAPTER_TITLES.len() as i32 {
            if self.story_unlocked(ix as usize){
                self.story_chapter=ix as usize;
                self.story_selection=ix as usize;
                self.story_page=0;self.story_page_count=1;
                self.story_auto=false;
                return;
            }
            ix+=delta.signum();
        }
    }
    pub fn story_next_page(&mut self){
        if self.story_page+1<self.story_page_count {
            self.story_page+=1;
        } else if self.story_auto && self.story_chapter==5 && self.finished {
            // The last fight transitions straight into the complete epilogue.
            self.story_chapter=6;self.story_page=0;self.story_page_count=1;
        } else {
            self.close_story();
        }
    }
    pub fn story_previous_page(&mut self){self.story_page=self.story_page.saturating_sub(1);}
    pub fn close_story(&mut self){
        self.screen=self.story_return;
        self.story_auto=false;
    }

    pub fn open_lore(&mut self,index:usize){
        if index>=lore::RECORDS.len(){return;}
        self.lore_mask|=1<<index;
        self.checkpoint.lore_mask=self.lore_mask;
        self.journal_entry=index;self.journal_page=0;
        self.screen=Screen::Journal;
        self.write_save();
    }
    pub fn open_journal(&mut self){
        if self.lore_mask==0{return}
        if self.lore_mask & (1<<self.journal_entry)==0 {
            self.journal_entry=(0..lore::RECORDS.len()).find(|&i|self.lore_mask&(1<<i)!=0).unwrap_or(0);
        }
        self.journal_page=0;self.screen=Screen::Journal;
    }
    pub fn navigate_journal(&mut self,step:i32){
        if self.screen!=Screen::Journal{return}
        for offset in 1..=lore::RECORDS.len(){
            let len=lore::RECORDS.len() as i32;
            let next=(self.journal_entry as i32+step*offset as i32).rem_euclid(len) as usize;
            if self.lore_mask & (1<<next)!=0 {self.journal_entry=next;self.journal_page=0;break}
        }
    }
    pub fn journal_page_step(&mut self,step:i32){
        if self.screen!=Screen::Journal{return}
        let pages=lore::RECORDS[self.journal_entry].lines.len().div_ceil(lore::LINES_PER_PAGE);
        self.journal_page=(self.journal_page as i32+step).clamp(0,pages.saturating_sub(1) as i32) as usize;
    }
    pub fn mission_cleared(&mut self){
        if self.screen!=Screen::Playing{return;}
        self.last_level_time=self.world.time;
        self.level_score=self.world.score.saturating_sub(self.checkpoint.score);
        let total_kills=self.completed_kills+self.world.kills;
        self.completed_kills=total_kills;
        if self.world.level+1==MISSIONS.len(){
            self.best_score=self.best_score.max(self.world.score);
            self.checkpoint=Progress{
                level:self.world.level,unlocked:self.world.level,score:self.world.score,
                health:self.world.health.max(45),ammo:self.world.ammo.max(24),
                kills:total_kills,best_score:self.best_score,finished:true,lore_mask:self.lore_mask,
            };
            self.finished=true;self.write_save();
            self.open_story_auto(5,Screen::Ending);
        } else {
            let next=self.world.level+1;
            self.checkpoint=Progress {
                level:next,unlocked:next,score:self.world.score,
                health:self.world.health.max(45),ammo:self.world.ammo.max(24),
                kills:total_kills,best_score:self.best_score,finished:self.finished,lore_mask:self.lore_mask,
            };
            self.write_save();
            self.open_story_auto(self.world.level+1,Screen::Debrief);
        }
    }
    pub fn next_mission(&mut self){
        if self.screen==Screen::Debrief{self.begin_checkpoint();}
    }
    pub fn mission_index(&self)->usize{self.world.level}
    pub fn saved_level(&self)->usize{self.checkpoint.level}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn save_codec_round_trip(){
        let p=Progress{level:4,unlocked:4,score:550,kills:33,health:72,ammo:61,best_score:888,finished:true,lore_mask:0x5555};
        assert_eq!(Progress::decode(&p.encode()),Some(p));
    }
    #[test] fn older_completed_save_unlocks_final_mission(){
        let p=Progress::decode("MATA_CAMPAIGN_V1\n3 3 900 53 28 24 2000 1\n").unwrap();
        assert_eq!(p.level,4);assert_eq!(p.unlocked,4);
        assert!(!p.finished);assert_eq!(p.health,75);assert_eq!(p.ammo,40);
    }
    #[test] fn corrupted_save_is_rejected(){
        assert_eq!(Progress::decode("MATA_CAMPAIGN_V2\n900 900 2 100 30 2 4 0 0\n"),None);
        assert_eq!(Progress::decode("MATA_CAMPAIGN_V2\n1 1 2 100 30 2 4 0 4294967295\n"),None);
    }
    #[test] fn mission_transitions_preserve_stats(){
        let mut c=Campaign::new();c.checkpoint=Progress::fresh();c.begin_checkpoint();c.screen=Screen::Playing;
        c.world.health=15;c.world.ammo=7;c.world.kills=4;c.world.score=900;
        c.save_file=std::env::temp_dir().join(format!("mata-raycast-test-{}.save",std::process::id()));
        c.mission_cleared();assert_eq!(c.screen,Screen::Story);
        assert_eq!(c.story_chapter,1);assert!(c.story_unlocked(1));
        assert!(!c.story_unlocked(2));
        c.close_story();assert_eq!(c.screen,Screen::Debrief);
        c.next_mission();assert_eq!(c.world.level,1);assert_eq!(c.world.health,45);assert_eq!(c.world.ammo,24);
        assert_eq!(c.world.score,900);assert_eq!(c.completed_kills,4);let _=fs::remove_file(c.save_file);
    }
    #[test] fn story_library_protects_future_spoilers(){
        let mut c=Campaign::new();
        c.checkpoint=Progress::fresh();c.lore_mask=0;c.finished=false;
        for n in 1..=8 {assert!(!c.story_unlocked(n));}
        c.open_library();assert_eq!(c.screen,Screen::StoryIndex);
        c.story_selection=4;c.story_open_selection();
        assert_eq!(c.screen,Screen::StoryIndex);
        c.story_selection=0;c.story_open_selection();
        assert_eq!(c.screen,Screen::Story);
        c.close_story();assert_eq!(c.screen,Screen::StoryIndex);
        c.close_library();assert_eq!(c.screen,Screen::Title);
        c.checkpoint.unlocked=3;
        for n in 1..=3 {assert!(c.story_unlocked(n));}
        assert!(c.story_unlocked(7));
        assert!(!c.story_unlocked(4));
        assert!(!c.story_unlocked(6));
        c.finished=true;
        for n in 0..=8 {assert!(c.story_unlocked(n));}
    }
    #[test] fn finale_story_leads_into_epilogue(){
        let mut c=Campaign::new();c.checkpoint=Progress::fresh();
        c.finished=true;c.open_story_auto(5,Screen::Ending);
        c.story_page_count=1;c.story_next_page();
        assert_eq!(c.story_chapter,6);
        c.story_next_page();assert_eq!(c.screen,Screen::Ending);
    }
    #[test] fn journal_entries_persist(){
        let mut c=Campaign::new();c.checkpoint=Progress::fresh();
        c.save_file=std::env::temp_dir().join(format!("mata-raycast-journal-{}.save",std::process::id()));
        c.open_lore(14);assert_eq!(c.screen,Screen::Journal);assert_eq!(c.lore_mask,1<<14);
        assert_eq!(Progress::decode(&fs::read_to_string(&c.save_file).unwrap()).unwrap().lore_mask,1<<14);
        let _=fs::remove_file(c.save_file);
    }
}
