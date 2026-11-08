//! MATA // OUTPOST ZERO - small original Doom-inspired raycast FPS.
//! Native window/framebuffer from mata_win, text from mata-ui.
mod render;
mod sound;
mod world;
mod levels;
mod campaign;
mod lore;
mod story;

use std::{time::{Duration,Instant},fs};
use mata_win::{Framebuffer, Window, events::{WindowEvent, Key}};
use mata_ui::widgets::FontRenderer;
use world::Mode;
use campaign::{Campaign,Screen};

fn has(window:&Window,ch:char)->bool {
    window.is_key_down(Key::Char(ch))||window.is_key_down(Key::Char(ch.to_ascii_uppercase()))
}

fn fonts()->Option<FontRenderer>{
    // Use mata_fonts to validate the font, and mata-ui's cached glyph renderer
    // (which also uses mata_fonts) to draw on the game framebuffer.
    let mut candidates=vec![
        String::from("C:/Windows/Fonts/arial.ttf"),String::from("C:/Windows/Fonts/segoeui.ttf"),
        String::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        String::from("/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf"),
        String::from("/usr/share/fonts/TTF/DejaVuSans.ttf"),
    ];
    if let Ok(windir)=std::env::var("WINDIR"){candidates.insert(0,format!("{windir}/Fonts/arial.ttf"));}
    for candidate in candidates{
        if let Ok(data)=fs::read(candidate){
            if mata_fonts::decoders::ttf::Ttf::new(&data).is_ok(){
                if let Ok(renderer)=FontRenderer::from_bytes(&data){return Some(renderer);}
            }
        }
    }
    FontRenderer::system_default().ok()
}

fn run(mut window:Window){
    let (width,height)=window.client_size();
    let mut framebuffer=Framebuffer::from_color(width.max(1),height.max(1),0xff091522);
    let mut renderer=render::Renderer::new();
    let mut font=fonts();
    let mut sounds=sound::Sounds::new();
    let mut campaign=Campaign::new();
    let mut events=Vec::with_capacity(32);
    let mut prior=Instant::now();
    let mut cursor_hidden=false;
    let mut mouse_last=None::<(i32,i32)>;
    let mut raw_mouse_delta=0i32;
    let mut mouse_ready=false;
    let mut show_map=true;
    let mut fps_smoothed=60.0f32;
    let mut fire_click=false;
    loop {
        let frame_start=Instant::now();
        let dt=(frame_start-prior).as_secs_f32().clamp(0.001,0.05);
        prior=frame_start;
        fps_smoothed=fps_smoothed*0.94+0.06*(1.0/dt);
        window.poll_events_into(&mut events);
        // The portable desktop MataWin API supports visibility, not confinement
        // or warping. Remember the previous mouse position for relative deltas.
        let should_hide=campaign.screen==Screen::Playing && window.cursor_focused();
        if should_hide!=cursor_hidden {
            if window.set_cursor_visible(!should_hide).is_ok() {
                cursor_hidden=should_hide;
            }
            mouse_last=None; // don't jump when switching menu/game or re-entering
            mouse_ready=false;
        }
        raw_mouse_delta=0;
        for event in events.iter().copied() {
            match event {
                WindowEvent::Close=>{let _=window.show_cursor();return},
                WindowEvent::KeyPressed(Key::Escape)=>match campaign.screen {
                    Screen::Title=>{let _=window.show_cursor();return},
                    Screen::Story=>campaign.close_story(),
                    Screen::StoryIndex=>campaign.close_library(),
                    Screen::Playing=>campaign.screen=Screen::Paused,
                    Screen::Paused|Screen::Journal=>campaign.screen=Screen::Playing,
                    _=>campaign.return_to_title(),
                },
                WindowEvent::KeyPressed(Key::Enter)=>match campaign.screen {
                    Screen::Title=>campaign.new_campaign(),
                    Screen::Story=>campaign.story_next_page(),
                    Screen::StoryIndex=>campaign.story_open_selection(),
                    Screen::Prologue=>campaign.screen=Screen::Briefing,
                    Screen::Briefing|Screen::Paused|Screen::Journal=>campaign.screen=Screen::Playing,
                    Screen::Debrief=>campaign.next_mission(),
                    Screen::GameOver=>campaign.retry(),
                    Screen::Ending=>campaign.return_to_title(),
                    Screen::Playing=>{},
                },
                WindowEvent::KeyPressed(Key::Char('b'))|WindowEvent::KeyPressed(Key::Char('B'))=>match campaign.screen {
                    Screen::Story=>campaign.close_story(),
                    Screen::StoryIndex=>campaign.close_library(),
                    _=>campaign.open_library(),
                },
                WindowEvent::KeyPressed(Key::Up) if campaign.screen==Screen::StoryIndex=>campaign.story_select(-1),
                WindowEvent::KeyPressed(Key::Down) if campaign.screen==Screen::StoryIndex=>campaign.story_select(1),
                WindowEvent::KeyPressed(Key::Up) if campaign.screen==Screen::Story=>campaign.story_previous_page(),
                WindowEvent::KeyPressed(Key::Down) if campaign.screen==Screen::Story=>campaign.story_next_page(),
                WindowEvent::KeyPressed(Key::Left) if campaign.screen==Screen::Story=>campaign.story_change_chapter(-1),
                WindowEvent::KeyPressed(Key::Right) if campaign.screen==Screen::Story=>campaign.story_change_chapter(1),
                WindowEvent::KeyPressed(Key::Space) if campaign.screen==Screen::Story=>campaign.story_next_page(),
                WindowEvent::KeyPressed(Key::Char('n'))|WindowEvent::KeyPressed(Key::Char('N')) if campaign.screen==Screen::Title=>{
                    campaign.new_campaign();
                },
                WindowEvent::KeyPressed(Key::Char('c'))|WindowEvent::KeyPressed(Key::Char('C')) if campaign.screen==Screen::Title=>{
                    campaign.continue_campaign();
                },
                WindowEvent::KeyPressed(Key::Char('m'))|WindowEvent::KeyPressed(Key::Char('M')) if campaign.screen!=Screen::Playing=>{
                    campaign.return_to_title();
                },
                WindowEvent::KeyPressed(Key::Char('r'))|WindowEvent::KeyPressed(Key::Char('R')) if matches!(campaign.screen,Screen::Playing|Screen::Paused|Screen::GameOver)=>{
                    campaign.retry();sounds.play("pickup");
                },
                WindowEvent::KeyPressed(Key::Char('e'))|WindowEvent::KeyPressed(Key::Char('E')) if campaign.screen==Screen::Playing=>{
                    if campaign.world.action(){sounds.play("door");}
                    if let Some(log)=campaign.world.take_lore(){
                        sounds.play("log");campaign.open_lore(log);
                    }
                },
                WindowEvent::KeyPressed(Key::Char('e'))|WindowEvent::KeyPressed(Key::Char('E')) if campaign.screen==Screen::Journal=>{
                    campaign.screen=Screen::Playing;
                },
                WindowEvent::KeyPressed(Key::Char('j'))|WindowEvent::KeyPressed(Key::Char('J'))=>match campaign.screen{
                    Screen::Playing|Screen::Paused=>campaign.open_journal(),
                    Screen::Journal=>campaign.screen=Screen::Playing,
                    _=>{},
                },
                WindowEvent::KeyPressed(Key::Left) if campaign.screen==Screen::Journal=>campaign.navigate_journal(-1),
                WindowEvent::KeyPressed(Key::Right) if campaign.screen==Screen::Journal=>campaign.navigate_journal(1),
                WindowEvent::KeyPressed(Key::Up) if campaign.screen==Screen::Journal=>campaign.journal_page_step(-1),
                WindowEvent::KeyPressed(Key::Down) if campaign.screen==Screen::Journal=>campaign.journal_page_step(1),
                WindowEvent::KeyPressed(Key::F(1))=>show_map=!show_map,
                WindowEvent::KeyPressed(Key::F(2))=>renderer.toggle_meshes(),
                WindowEvent::KeyPressed(Key::F(3))=>sounds.toggle(),
                WindowEvent::KeyPressed(Key::MouseLeft) if campaign.screen==Screen::Playing=>fire_click=true,
                WindowEvent::KeyPressed(Key::MouseRight) if campaign.screen==Screen::Playing=>campaign.world.aim_x=0.0,
                WindowEvent::CursorMoved(mx,my)=>{
                    if campaign.screen==Screen::Playing {
                        if let Some((last,_))=mouse_last {
                            raw_mouse_delta=raw_mouse_delta.saturating_add((mx-last).clamp(-75,75));
                            mouse_ready=true;
                        }
                    }
                    mouse_last=Some((mx,my));
                }
                _=>{}
            }
        }
        // Pause/story/mission transitions within this event batch must also
        // restore the system pointer before showing their UI.
        let should_hide=campaign.screen==Screen::Playing && window.cursor_focused();
        if should_hide!=cursor_hidden {
            if window.set_cursor_visible(!should_hide).is_ok() {
                cursor_hidden=should_hide;
            }
            mouse_last=None;
            mouse_ready=false;
            raw_mouse_delta=0;
        }
        if campaign.screen==Screen::Playing {
            let world=&mut campaign.world;
            if mouse_ready {world.aim(raw_mouse_delta as f32,window.client_size().0 as f32);}
            let turn=(window.is_key_down(Key::Right) as i32-window.is_key_down(Key::Left) as i32) as f32;
            world.look(turn*dt*2.25);
            let forward=((has(&window,'w')||window.is_key_down(Key::Up)) as i32-(has(&window,'s')||window.is_key_down(Key::Down)) as i32) as f32;
            let strafe=(has(&window,'d') as i32-has(&window,'a') as i32) as f32;
            world.moving=forward!=0.0||strafe!=0.0;
            if world.moving {
                let mag=(forward*forward+strafe*strafe).sqrt().max(1.0);
                let speed=if window.is_key_down(Key::LeftShift)||window.is_key_down(Key::RightShift){4.2}else{2.9};
                let dx=(forward*world.facing.cos()-strafe*world.facing.sin())/mag*dt*speed;
                let dy=(forward*world.facing.sin()+strafe*world.facing.cos())/mag*dt*speed;
                world.move_to(dx,dy);
            }
            if fire_click||window.is_key_down(Key::Space)||window.is_key_down(Key::MouseLeft){
                let boss_alive=world.boss_alive();
                if world.shoot(){sounds.play("shoot");}
                if boss_alive&&!world.boss_alive(){sounds.play("boss_down");}
            }
            let old_score=world.score;
            let old_health=world.health;
            let old_phase=world.boss_phase;
            let old_blast=world.boss_flash;
            world.tick(dt);
            if world.boss_phase>old_phase && world.boss_phase!=4{sounds.play("charge");}
            if world.boss_flash>old_blast {sounds.play("blast");}
            if world.score>old_score{sounds.play("pickup");}
            if world.health<old_health{sounds.play("hurt");}
            match world.mode {
                Mode::Won=>{sounds.play("win");campaign.mission_cleared();},
                Mode::Lost=>campaign.screen=Screen::GameOver,
                Mode::Playing=>{},
            }
        }
        fire_click=false;
        let (cw,ch)=window.client_size();
        if cw==0||ch==0 {std::thread::sleep(Duration::from_millis(10));continue}
        if cw!=framebuffer.width||ch!=framebuffer.height {
            framebuffer.resize_and_clear(cw,ch,0xff081320);
        }
        renderer.render(&mut framebuffer,&mut campaign,&mut font,show_map,fps_smoothed);
        if !window.present_exact(&framebuffer){
            let (cw,ch)=window.client_size();
            if cw>0&&ch>0 {
                framebuffer.resize_and_clear(cw,ch,0xff081320);
                renderer.render(&mut framebuffer,&mut campaign,&mut font,show_map,fps_smoothed);
                window.present(&framebuffer);
            }
        }
        let frame_length=frame_start.elapsed();
        let target=Duration::from_secs_f32(1.0/60.0);
        if frame_length<target{std::thread::sleep(target-frame_length);}
    }
}
fn main(){run(Window::new("MATA // OUTPOST ZERO - Campaign",(960,600)));}
