//! Mineral Empire: an original incremental mining game built with Mata crates.
pub mod big;
pub mod engine;
mod save;
mod slots;
mod sound;
mod ui;

use std::{path::PathBuf,thread,time::{Duration,Instant}};
use mata_win::{Window,Framebuffer,events::{WindowEvent,Key}};
use mata_ui::widgets::FontRenderer;
use engine::{Game,number};
use ui::{Interface,Action};

fn run(mut window:Window,legacy_save_path:PathBuf){
    let save_root=legacy_save_path.parent().unwrap_or_else(||std::path::Path::new(".")).to_path_buf();
    let mut active_slot=slots::current(&save_root);
    let mut save_path=slots::path(&save_root,active_slot);
    let mut font=FontRenderer::from_bytes(include_bytes!("../assets/fonts/DejaVuSans.ttf"))
        .expect("embedded game font should be valid");
    let (mut game, offline, mut notice, mut slot_writable)=match save::load(&save_path){
        Ok((g,sec)) => (g,sec,format!("Welcome back. {}h {}m of offline mining credited.",sec/3600,(sec%3600)/60),true),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound => (Game::default(),0,"Open a save slot, or create a new expedition.".into(),true),
        Err(e) => (Game::default(),0,format!("Slot {active_slot:02} could not be read: {e}. The file is protected; load another slot or reset it."),false),
    };
    // Offline production is capped by the save layer, and never repeated while the app is running.
    let _=offline;
    let cache=save_path.parent().unwrap_or_else(||std::path::Path::new("."));
    let mut sounds=sound::Sounds::new(&cache.join("sound"));
    let mut ui=Interface::new();ui.notice=std::mem::take(&mut notice);
    ui.update_slots(slots::list(&save_root,active_slot),active_slot);
    let mut cursor=(0i32,0i32);
    let mut pointer_down:Option<(i32,i32)>=None;
    let mut dragged=false;
    let mut last=Instant::now();let mut last_render=Instant::now()-Duration::from_millis(200);
    let mut autosave=Instant::now();let mut auto_poll=Instant::now();
    let mut dirty=true;
    // The native client size is authoritative. `framebuffer_size` describes
    // the *previously submitted* surface and can lag WM_SIZE or maximize.
    // Rendering against it causes Direct2D to stretch the old bitmap, while
    // cursor events remain in current native-client coordinates.
    let (width,height)=window.client_size();
    let mut fb=Framebuffer::from_color(width.max(1),height.max(1),0xff0b101b);
    let mut events=Vec::with_capacity(32);
    loop {
        window.poll_events_into(&mut events);
        // Rebuild the UI layout at the native size before processing input.
        // A resize and a mouse release can arrive in the same event batch.
        if resize_to_client(&window,&mut fb) {
            ui.draw(&mut fb,&mut font,&game);
            dirty=true;
        }
        let mut exit=false;
        for event in events.iter().copied(){
            match event {
                WindowEvent::Close=>exit=true,
                WindowEvent::KeyPressed(key) if !ui.menu_open && ui.min_gain_editing && !matches!(key,Key::MouseLeft)=>{
                    if let Some(result)=ui.input_min_gain(key,&mut game){
                        if let Err(message)=result{ui.notice=message;}
                        else if !ui.min_gain_editing {
                            if matches!(key,Key::Escape) {
                                ui.notice="Minimum gain editing cancelled.".into();
                            } else {
                                ui.notice=format!("Minimum auto-prestige gain: {}",number(game.auto_prestige_target_gain()));
                                if let Err(e)=save_if_writable(&save_path,&game,slot_writable){ui.notice=format!("Could not save gain: {e}");}
                            }
                        }
                        dirty=true;
                    }
                }
                WindowEvent::KeyPressed(Key::Escape)=>{
                    if ui.menu_open {
                        if ui.menu_reset_confirm {ui.menu_reset_confirm=false;}
                        else if slot_writable {ui.resume();}
                    }else {
                        ui.open_menu();
                        ui.update_slots(slots::list(&save_root,active_slot),active_slot);
                    }
                    dirty=true;
                },
                WindowEvent::CursorMoved(x,y)=>{
                    if let Some((ox,oy))=pointer_down {
                        if dragged || (x-ox).abs()+(y-oy).abs()>12 {
                            ui.drag_scroll((y-oy) as f32);
                            dragged=true;
                            dirty=true;
                        }
                        pointer_down=Some((x,y));
                    }
                    cursor=(x,y);
                }
                WindowEvent::Scroll(_,dy)=>{ui.scroll(dy);dirty=true;}
                WindowEvent::KeyPressed(Key::MouseLeft)=>{pointer_down=Some(cursor);dragged=false;}
                WindowEvent::KeyReleased(Key::MouseLeft)=>{
                    if pointer_down.take().is_some() && !dragged {
                        // Do not use last frame's button rectangles if the
                        // window resized since events were dequeued.
                        let resized=resize_to_client(&window,&mut fb);
                        if resized || dirty {
                            // Scroll, tabs, or resize may have changed layout
                            // since the last presented frame. Refresh hitboxes
                            // before looking up this click.
                            ui.draw(&mut fb,&mut font,&game);
                            dirty=true;
                        }
                        if let Some(action)=ui.hit(cursor.0,cursor.1){
                            if !matches!(action,Action::EditMinGain|Action::GainChar(_)|Action::GainBackspace|Action::GainCommit|Action::GainCancel) && ui.min_gain_editing {
                                if let Err(error)=ui.commit_min_gain(&mut game){
                                    ui.notice=error;
                                    dirty=true;
                                    continue;
                                }
                                let _=save_if_writable(&save_path,&game,slot_writable);
                            }
                            apply_action(action,&mut game,&mut ui,&mut sounds,
                                &save_root,&mut save_path,&mut active_slot,&mut slot_writable);
                            dirty=true;
                        }
                    }
                    dragged=false;
                }
                WindowEvent::KeyPressed(Key::Char('s')) | WindowEvent::KeyPressed(Key::Char('S'))=>{
                    if !ui.menu_open {apply_action(Action::Save,&mut game,&mut ui,&mut sounds,
                        &save_root,&mut save_path,&mut active_slot,&mut slot_writable);}
                    dirty=true;
                }
                WindowEvent::Resized(_,_)|WindowEvent::Maximized(_,_)|WindowEvent::Restored(_,_)=>{
                    // These event dimensions may already be stale; use the
                    // actual client size at the point of rendering instead.
                    dirty=true;
                }
                _=>{}
            }
        }
        if exit {break;}
        let now=Instant::now();
        let dt=now.duration_since(last).as_secs_f64().min(1.0);
        last=now;
        if !ui.menu_open { game.tick(dt); }
        if !ui.menu_open && auto_poll.elapsed()>=Duration::from_millis(800) {
            // The engine's shared clock prevents purchases more frequently
            // than once per active minute, independent of this poll interval.
            // Reset eligible lower tiers before spending run cash, so the
            // optimizer never buys upgrades right before discarding the mines.
            if let Some((stage,gain))=game.auto_prestige_tick(){
                ui.pending=None;
                ui.notice=format!("Auto-prestige: +{} {}",number(&gain),engine::STAGES[stage].currency);
                sounds.play(1);
                if let Err(e)=save_if_writable(&save_path,&game,slot_writable){ui.notice=format!("Auto-prestige succeeded; save failed: {e}");}
            }
            game.auto_research_tick();
            game.auto_tick();
            auto_poll=now;dirty=true;
        }
        if !ui.menu_open && autosave.elapsed()>=Duration::from_secs(25){
            if let Err(e)=save_if_writable(&save_path,&game,slot_writable){ui.notice=format!("Autosave failed: {e}");}
            autosave=now;
        }
        if resize_to_client(&window,&mut fb) {dirty=true;}
        // Never present a framebuffer of a different size and let the window
        // backend stretch its old pixels. Every frame is rasterized anew.
        if (dirty || last_render.elapsed() >= Duration::from_millis(150))
            && fb.width>0 && fb.height>0
        {
            ui.draw(&mut fb,&mut font,&game);
            if window.present_exact(&fb) {
                last_render=now;
                dirty=false;
            } else {
                // The native surface changed between draw and present; retry
                // on the next iteration with the updated client size.
                dirty=true;
            }
        }
        thread::sleep(Duration::from_millis(16));
    }
    let _=save_if_writable(&save_path,&game,slot_writable);
}
fn save_if_writable(path:&std::path::Path,game:&Game,writable:bool)->std::io::Result<()> {
    if !writable {return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied,
        "current save is unreadable; load another slot or confirm reset to preserve it"));}
    save::save(path,game)
}
/// Keep the software surface 1:1 with the drawable client area. In particular,
/// don't use `Window::framebuffer_size()` for this: on Windows it represents the
/// last presented surface, not the latest HWND client dimensions.
fn resize_to_client(window:&Window,fb:&mut Framebuffer)->bool {
    let (w,h)=window.client_size();
    if w==0 || h==0 || (w,h)==(fb.width,fb.height) {return false;}
    fb.resize_and_clear(w,h,0xff0b101b);
    true
}

fn apply_action(action:Action,game:&mut Game,ui:&mut Interface,sound:&mut sound::Sounds,
    save_root:&std::path::Path,save_path:&mut PathBuf,active_slot:&mut u32,slot_writable:&mut bool){
    match action {
        Action::Tab(tab)=>{ui.tab=tab;ui.scroll=0.0;ui.pending=None;},
        Action::Quantity(q)=>ui.amount=q,
        Action::Miner(i)=>{
            let amount=game.buy_miner(i,ui.amount);
            if amount>0{ui.notice=format!("Hired {} {} miners",amount,engine::ORES[i].name);sound.play(0);}
            else{ui.notice="Not enough credits for that hire.".into();}
        }
        Action::Equipment(i)=>{
            if game.buy_equipment(i){ui.notice=format!("{} extraction equipment upgraded!",engine::ORES[i].name);sound.play(2);}
            else{ui.notice="You need more credits or at least one worker.".into();}
        }
        Action::Ascend(level)=>{
            if game.prestige_available(level){
                if ui.pending!=Some(level){
                    ui.pending=Some(level);
                    ui.notice="Press CONFIRM RESET to cash out your prestige. All mines will restart.".into();
                }else{
                    let gained=game.ascend(level);
                    ui.pending=None;
                    ui.notice=format!("Ascended! +{} {}. Your empire has expanded.",number(&gained),engine::STAGES[level].currency);
                    sound.play(1);
                    let _=save_if_writable(save_path,game,*slot_writable);
                }
            }else{ui.notice="This prestige tier is still locked.".into();}
        }
        Action::Research(t,k)=>{
            let bought=game.buy_research_bulk(t,k,ui.research_amount.count());
            if bought>0 {
                ui.notice=format!("{} / {}: purchased {} research ranks",engine::STAGES[t].name,engine::research_name(t,k),bought);
                sound.play(2);
            }else{ui.notice="Not enough prestige currency for that research.".into();}
        }
        Action::ResearchQuantity(q)=>{ui.research_amount=q;ui.notice=format!("Research purchases: up to {} levels per click.",q.count());}
        Action::ToggleAutoResearch=>{
            if game.auto_research_unlocked(){game.auto_research=!game.auto_research;
                ui.notice=format!("Auto-research {}.",if game.auto_research{"enabled"}else{"disabled"});
            } else {ui.notice="Purchase Auto-Research at any prestige tier to unlock automation.".into();}
        }
        Action::ToggleAuto=>{
            if game.auto_hire_unlocked(){
                game.auto_buy=!game.auto_buy;
                game.auto_economy_elapsed=0.;
                ui.notice=format!("Auto-hire {}. Income-optimized purchases every 60 seconds.",
                    if game.auto_buy{"enabled"}else{"disabled"});
            }else{ui.notice="Buy Automation research to enable auto-hiring.".into();}
        }
        Action::ToggleGear=>{
            if game.auto_gear_unlocked(){
                game.auto_gear=!game.auto_gear;
                game.auto_economy_elapsed=0.;
                ui.notice=format!("Auto-gear {}. Income-optimized purchases every 60 seconds.",
                    if game.auto_gear{"enabled"}else{"disabled"});
            }else{ui.notice="Buy Auto-Gear research to automate equipment upgrades.".into();}
        }
        Action::ToggleAutoPrestige=>{
            let current=game.current_prestige_tier();
            if current==0 {
                ui.notice="Reach at least the second prestige tier first.".into();
            } else if game.auto_prestige_unlocked(){
                game.auto_prestige=!game.auto_prestige;
                game.auto_prestige_elapsed=0.;
                ui.notice=format!("Auto-prestige {} for tiers 1-{}; current tier {} stays manual.",
                    if game.auto_prestige{"enabled"}else{"disabled"},current,current+1);
            } else {ui.notice="Research Auto-Prestige at a prestige tier first.".into();}
        }
        Action::EditMinGain=>{
            ui.focus_min_gain(game);
            ui.notice="Type a positive minimum gain, e.g. 1000 or 1e5000. Press Enter to save.".into();
        }
        Action::GainChar(ch)=>{
            let _=ui.input_min_gain(Key::Char(ch),game);
        }
        Action::GainBackspace=>{
            let _=ui.input_min_gain(Key::Backspace,game);
        }
        Action::GainCommit=>match ui.commit_min_gain(game) {
            Ok(())=>{
                ui.notice=format!("Minimum auto-prestige gain: {}",number(game.auto_prestige_target_gain()));
                if let Err(e)=save_if_writable(save_path,game,*slot_writable){ui.notice=format!("Could not save gain: {e}")}
            }
            Err(err)=>ui.notice=err,
        },
        Action::GainCancel=>{
            let _=ui.input_min_gain(Key::Escape,game);
            ui.notice="Minimum gain editing cancelled.".into();
        }
        Action::ToggleSound=>{sound.toggle();ui.sound_enabled=sound.enabled();}
        Action::Save=>match save_if_writable(save_path,game,*slot_writable){
            Ok(())=>{ui.notice="Empire saved successfully.".into();sound.play(2);}
            Err(e)=>ui.notice=format!("Save failed: {e}"),
        },
        Action::MenuOpen=>{
            ui.open_menu();ui.update_slots(slots::list(save_root,*active_slot),*active_slot);
            ui.notice=format!("Managing save slot {:02}.",*active_slot);
        }
        Action::MenuResume=>{
            if *slot_writable {ui.resume();ui.notice=format!("Playing slot {:02}.",*active_slot);}
            else{ui.notice="This slot is unreadable. Load another slot or confirm RESET CURRENT.".into();}
        }
        Action::MenuSave=>match save_if_writable(save_path,game,*slot_writable){
            Ok(())=>{ui.notice=format!("Slot {:02} saved.",*active_slot);sound.play(2);}
            Err(e)=>ui.notice=format!("Save failed: {e}"),
        },
        Action::MenuNewSlot=>{
            let result=(||->std::io::Result<()> {
                // A damaged slot remains untouched; allow a fresh slot as a recovery path.
                if *slot_writable {save_if_writable(save_path,game,true)?;}
                let id=slots::create(save_root)?;
                slots::set_current(save_root,id)?;
                *active_slot=id;*save_path=slots::path(save_root,id);
                *game=Game::default();*slot_writable=true;
                Ok(())
            })();
            match result {
                Ok(())=>{ui.tab=ui::Tab::Mines;ui.scroll=0.;ui.pending=None;
                    ui.notice=format!("Created slot {:02}. Start your new expedition!",*active_slot);
                    ui.resume();sound.play(2);}
                Err(e)=>ui.notice=format!("New slot failed: {e}"),
            }
        }
        Action::MenuLoadSlot(id)=>{
            if id==*active_slot && *slot_writable {
                ui.resume();ui.notice=format!("Playing slot {:02}.",id);
            } else if !(1..=slots::MAX_SLOTS).contains(&id) {
                ui.notice="Invalid save slot.".into();
            } else {
                let result=(||->std::io::Result<(Game,u64)> {
                    // Do not save or overwrite an unreadable source slot.
                    if *slot_writable {save_if_writable(save_path,game,true)?;}
                    let candidate=slots::path(save_root,id);
                    let loaded=match save::load(&candidate){
                        Ok(state)=>state,
                        Err(e) if e.kind()==std::io::ErrorKind::NotFound =>(Game::default(),0),
                        Err(e)=>return Err(e.into()),
                    };
                    slots::set_current(save_root,id)?;
                    Ok(loaded)
                })();
                match result {
                    Ok((loaded,offline))=>{
                        *game=loaded;*active_slot=id;*save_path=slots::path(save_root,id);
                        *slot_writable=true;ui.tab=ui::Tab::Mines;ui.scroll=0.;ui.pending=None;
                        ui.resume();ui.notice=format!("Loaded slot {:02} ({}m offline earnings).",id,offline/60);
                        sound.play(2);
                    }
                    Err(e)=>ui.notice=format!("Could not load slot {id:02}: {e}"),
                }
            }
        }
        Action::MenuResetAsk=>{ui.menu_reset_confirm=true;},
        Action::MenuResetCancel=>{ui.menu_reset_confirm=false;},
        Action::MenuResetConfirm=>{
            match slots::reset(save_root,*active_slot){
                Ok(())=>{
                    *game=Game::default();*slot_writable=true;ui.menu_reset_confirm=false;
                    ui.pending=None;ui.scroll=0.;ui.tab=ui::Tab::Mines;
                    ui.notice=format!("Slot {:02} reset. Previous progress backed up.",*active_slot);
                    ui.resume();sound.play(2);
                }
                Err(e)=>{ui.menu_reset_confirm=false;ui.notice=format!("Could not reset slot: {e}");}
            }
        },
    }
    if ui.menu_open {ui.update_slots(slots::list(save_root,*active_slot),*active_slot);}
}

#[cfg(not(target_os="android"))]
pub fn run_desktop() {
    let window=Window::new("Mineral Empire - The Deep Frontier",(1220,800));
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    window.set_icon_bytes(include_bytes!("../icons/icon.ico"));
    #[cfg(all(target_os = "windows", debug_assertions))]
    {
        let icon_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons").join("icon.ico");
        window.set_icon(icon_path);
    }
    run(window,save::default_path());
}

#[cfg(target_os="android")]
#[unsafe(no_mangle)]
pub fn android_main(app:mata_win::AndroidApp){
    let dir=app.internal_data_path().unwrap_or_else(||std::env::current_dir().unwrap_or_else(|_|".".into()));
    let window=Window::from_android_app(app,"Mineral Empire",(500,1000));
    run(window,dir.join("save.dat"));
}
