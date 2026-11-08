//! Independent save slots built around the existing CRC-protected V2-V6 format.
//! Slot 1 remains `save.dat` so existing progress is discovered automatically.
use std::{fs,io,io::Write,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use crate::{engine::Game,save};

pub const MAX_SLOTS:u32=64;
#[derive(Clone,Debug)]
pub struct SlotInfo {pub id:u32,pub bytes:u64,pub is_active:bool}

pub fn path(root:&Path,id:u32)->PathBuf {
    assert!((1..=MAX_SLOTS).contains(&id),"invalid slot id");
    if id==1 {root.join("save.dat")}else{root.join(format!("slot-{id:02}.dat"))}
}
pub fn current(root:&Path)->u32 {
    fs::read_to_string(root.join("active-slot.txt")).ok()
        .and_then(|value|value.trim().parse().ok())
        .filter(|id| (1..=MAX_SLOTS).contains(id)).unwrap_or(1)
}
pub fn set_current(root:&Path,id:u32)->io::Result<()> {
    if !(1..=MAX_SLOTS).contains(&id) {return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid slot number"))}
    fs::create_dir_all(root)?;
    let tmp=root.join("active-slot.txt.tmp");
    {let mut f=fs::File::create(&tmp)?;writeln!(f,"{id}")?;f.sync_all()?;}
    // The pointer is not a game save. Losing it falls back to slot 1.
    #[cfg(windows)] if root.join("active-slot.txt").exists() {fs::remove_file(root.join("active-slot.txt"))?;}
    fs::rename(tmp,root.join("active-slot.txt"))
}
pub fn list(root:&Path,active:u32)->Vec<SlotInfo> {
    (1..=MAX_SLOTS).filter_map(|id|{
        let bytes=fs::metadata(path(root,id)).ok()?.len();
        Some(SlotInfo{id,bytes,is_active:id==active})
    }).collect()
}
pub fn next_empty(root:&Path)->io::Result<u32> {
    (2..=MAX_SLOTS).find(|&id|!path(root,id).exists())
        .ok_or_else(||io::Error::new(io::ErrorKind::Other,"all 64 save slots are occupied"))
}
pub fn create(root:&Path)->io::Result<u32> {
    fs::create_dir_all(root)?;
    let id=next_empty(root)?;
    // Never overwrite an existing slot. `save()` writes the normal V6 format.
    save::save(&path(root,id),&Game::default())?;
    Ok(id)
}
/// Reset is intentionally non-destructive: the prior save is retained with a
/// unique reset-backup name. The original game state is not replaced unless
/// writing the new save succeeds.
pub fn reset(root:&Path,id:u32)->io::Result<()> {
    let file=path(root,id);
    if file.exists() {
        let timestamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let mut backup=root.join(format!("slot-{id:02}-before-reset-{timestamp}.bak"));
        let mut suffix=0u32;
        while backup.exists() {
            suffix+=1;
            backup=root.join(format!("slot-{id:02}-before-reset-{timestamp}-{suffix}.bak"));
        }
        fs::copy(&file,&backup)?;
    }
    save::save(&file,&Game::default())
}
#[cfg(test)]mod tests {
    use super::*;
    fn test_dir(label:&str)->PathBuf {
        let base=std::env::temp_dir().join(format!("mineral-slot-{label}-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&base).unwrap();base
    }
    #[test] fn old_save_is_slot_one_and_slots_are_isolated(){
        let root=test_dir("migration");
        let mut old=Game::default();old.cash=crate::big::Big::from(12345.);
        save::save(&root.join("save.dat"),&old).unwrap();
        assert_eq!(path(&root,1),root.join("save.dat"));
        assert_eq!(current(&root),1);
        let second=create(&root).unwrap();assert_eq!(second,2);
        assert_eq!(save::load(&path(&root,1)).unwrap().0.cash,old.cash);
        assert_eq!(save::load(&path(&root,2)).unwrap().0.cash,Game::default().cash);
        set_current(&root,2).unwrap();assert_eq!(current(&root),2);
        reset(&root,1).unwrap();
        assert_eq!(save::load(&path(&root,1)).unwrap().0.cash,Game::default().cash);
        assert!(fs::read_dir(&root).unwrap().any(|f|f.unwrap().file_name().to_string_lossy().ends_with(".bak")));
        fs::remove_dir_all(root).unwrap();
    }
}
