//! Mata Hole: a complete, asset-free arcade game using MataWin and MataFonts.
mod game;
mod paint;
mod view;

use std::{collections::HashSet,thread,time::{Duration,Instant}};
use mata_win::{Window,Framebuffer,events::{Key,WindowEvent}};
use game::{Game,Input,Phase,V};
use paint::{Font,Painter};
use view::View;

fn held(keys:&HashSet<Key>,name:char)->bool {
    keys.contains(&Key::Char(name)) || keys.contains(&Key::Char(name.to_ascii_uppercase()))
}
fn movement(keys:&HashSet<Key>,mouse:Option<(i32,i32)>,w:i32,h:i32)->V {
    let mut x=0.0;let mut y=0.0;
    if held(keys,'a')||keys.contains(&Key::Left){x-=1.0;}
    if held(keys,'d')||keys.contains(&Key::Right){x+=1.0;}
    if held(keys,'w')||keys.contains(&Key::Up){y-=1.0;}
    if held(keys,'s')||keys.contains(&Key::Down){y+=1.0;}
    let v=V::new(x,y);
    if v.len()>0.01{return v.normalized();}
    if let Some((px,py))=mouse {
        let direction=V::new((px-w/2) as f32,(py-h/2) as f32);
        if direction.len()>36.0 {return direction.normalized();}
    }
    V::default()
}
fn start_or_restart(game:&mut Game){
    match game.phase {
        Phase::Title=>game.start(),
        Phase::Finished=>{*game=Game::new();game.start();},
        Phase::Playing=>{if game.paused {game.paused=false;}}
    }
}
fn game_key(k:Key,game:&mut Game) {
    match k {
        Key::Enter=>start_or_restart(game),
        Key::Space|Key::Escape=>{
            if game.phase==Phase::Title||game.phase==Phase::Finished {start_or_restart(game);}
            else{game.toggle_pause();}
        }
        Key::Char(c) if c.eq_ignore_ascii_case(&'r') && game.phase==Phase::Finished=>{
            *game=Game::new();game.start();
        }
        _=>{}
    }
}
fn run(mut window:Window){
    let mut game=Game::new();let mut font=Font::new();
    let mut view=View::new(game.holes[0].pos);
    let mut fb=Framebuffer::from_color(1280,800,0xFF152632);
    let mut events=Vec::with_capacity(96);
    let mut held_keys=HashSet::new();
    let mut mouse=None;
    let mut previous=Instant::now();let mut accumulator=0.0_f32;
    let fixed=1.0/60.0;
    loop {
        window.poll_events_into(&mut events);
        for event in events.iter().copied(){
            match event {
                WindowEvent::Close=>return,
                WindowEvent::CursorMoved(x,y)=>{mouse=Some((x,y));}
                WindowEvent::KeyPressed(Key::MouseLeft)=>{
                    if game.phase!=Phase::Playing||game.paused{start_or_restart(&mut game);}
                }
                WindowEvent::KeyPressed(k)=>{
                    // Some backends auto-repeat KeyPressed; process toggle actions only once.
                    if held_keys.insert(k){game_key(k,&mut game);}
                }
                WindowEvent::KeyReleased(k)=>{held_keys.remove(&k);}
                _=>{}
            }
        }
        let dims=window.client_size();
        let w=dims.0.min(5000);let h=dims.1.min(3500);
        if w==0||h==0 {thread::sleep(Duration::from_millis(35));previous=Instant::now();continue;}
        // Render at the actual framebuffer size. Never stretch cached text/sprites.
        if w!=fb.width || h!=fb.height {
            fb=Framebuffer::from_color(w,h,0xFF152632);
        }
        let now=Instant::now();let dt=now.duration_since(previous).as_secs_f32().clamp(0.0,0.10);
        previous=now;
        accumulator=(accumulator+dt).min(0.18);
        let input=Input{
            movement:movement(&held_keys,mouse,w as i32,h as i32),
            boosting:held_keys.contains(&Key::LeftShift)||held_keys.contains(&Key::RightShift),
        };
        while accumulator>=fixed {
            game.update(fixed,input);
            accumulator-=fixed;
        }
        view.update(&game,dt,w as i32,h as i32);
        {let mut painter=Painter::new(&mut fb,&mut font);view.draw(&mut painter,&game,mouse);}
        let _=window.present_exact(&fb);
        let used=Instant::now().duration_since(now);
        if used<Duration::from_micros(16_667){thread::sleep(Duration::from_micros(16_667)-used);}
    }
}

#[cfg(not(target_os="android"))]
fn main(){run(Window::new("Mata Hole | Swallow the City",(1280,800)));}

#[cfg(target_os="android")]
#[unsafe(no_mangle)]
pub fn android_main(app:mata_win::AndroidApp){
    run(Window::from_android_app(app,"Mata Hole",(1280,800)));
}
