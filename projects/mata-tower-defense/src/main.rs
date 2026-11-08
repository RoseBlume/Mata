//! Aegis: Grid Defense
//! Native window + framebuffer: mata_win
//! Font rasterization: mata_fonts
//! Simulation, controls, and sprites: Rust standard library only.
mod game;
mod paint;
mod view;

use std::{thread,time::{Duration,Instant}};
use mata_win::{Framebuffer,Window,events::{Key,WindowEvent}};
use game::{CANVAS_W,CANVAS_H,Game,TowerKind,Phase};
use paint::{Font,Painter,Viewport};

fn key_action(key:Key,game:&mut Game){
    match key {
        Key::Num(n) if (1..=4).contains(&n) => game.choose_build(TowerKind::ALL[(n-1) as usize]),
        Key::Char(ch) => match ch.to_ascii_lowercase() {
            '1'=>game.choose_build(TowerKind::Bolt),
            '2'=>game.choose_build(TowerKind::Cannon),
            '3'=>game.choose_build(TowerKind::Frost),
            '4'=>game.choose_build(TowerKind::Prism),
            'p'=>{if game.phase==Phase::Running {game.paused=!game.paused;}},
            'u'=>game.upgrade_selected(),
            'x'=>game.sell_selected(),
            'r'=>{if matches!(game.phase,Phase::Won|Phase::Lost) {*game=Game::new()}},
            _=>{},
        },
        Key::Space =>game.start_wave(),
        Key::Tab =>game.double_speed=!game.double_speed,
        Key::Escape=>{game.build=None;game.selected=None;},
        _=>{},
    }
}
fn click(x:i32,y:i32,g:&mut Game){
    if matches!(g.phase,Phase::Lost|Phase::Won){
        if within(x,y,557,518,326,59){*g=Game::new();}
        return;
    }
    if within(x,y,1008,31,111,51){if g.phase==Phase::Running {g.paused=!g.paused;}return;}
    if within(x,y,1130,31,109,51){g.double_speed=!g.double_speed;return;}
    if g.paused {return;}
    for (i,k) in TowerKind::ALL.iter().enumerate(){
        if within(x,y,1050,208+i as i32*97,340,87){g.choose_build(*k);return;}
    }
    if within(x,y,1052,773,340,65){g.start_wave();return;}
    if g.selected.is_some(){
        if within(x,y,1053,686,193,66){g.upgrade_selected();return;}
        if within(x,y,1252,686,134,66){g.sell_selected();return;}
    }
    if let Some((col,row))=game::tile_at(x as f32,y as f32){g.click_grid(col,row);}
}
fn within(x:i32,y:i32,bx:i32,by:i32,w:i32,h:i32)->bool {x>=bx&&y>=by&&x<bx+w&&y<by+h}
/// Use the same letterboxed viewport for painting, hovering and clicks.
/// All coordinates in the simulation remain on the 1440x900 logical canvas.
fn map_pointer(x:i32,y:i32,client_size:(u32,u32))->Option<(i32,i32)> {
    Viewport::new(client_size,(CANVAS_W as u32,CANVAS_H as u32))?
        .pointer(x,y,(CANVAS_W as u32,CANVAS_H as u32))
}

fn run(mut window:Window) {
    let mut game=Game::new();
    let mut font=Font::new();
    // Backbuffer is always the current native client size, never 1440x900
    // stretched by the presentation backend. Minimized windows use 1x1.
    let initial_size=window.client_size();
    let mut fb=Framebuffer::from_color(initial_size.0.max(1),initial_size.1.max(1),0xFF0B141F);
    let mut events=Vec::with_capacity(40);
    // Save the pointer in native client coordinates. Never cache the mapped
    // point: a resize can invalidate it without generating another move event.
    let mut raw_mouse:Option<(i32,i32)>=None;
    let mut last=Instant::now();
    let mut accumulated=0.0;
    let mut previous_frame=Instant::now();
    loop {
        window.poll_events_into(&mut events);
        for event in events.iter().copied(){
            match event {
                WindowEvent::Close=>return,
                WindowEvent::CursorMoved(x,y)=>{raw_mouse=Some((x,y));},
                WindowEvent::KeyPressed(Key::MouseLeft)=>{
                    // Ask for the CURRENT native size, not the last presented
                    // framebuffer size or the dimensions from an old event.
                    if let Some((x,y))=raw_mouse.and_then(|(x,y)|map_pointer(x,y,window.client_size())) {
                        click(x,y,&mut game);
                    }
                },
                WindowEvent::KeyPressed(Key::MouseRight)=>{game.build=None;game.selected=None;},
                WindowEvent::KeyPressed(key)=>key_action(key,&mut game),
                WindowEvent::Resized(_,_)|WindowEvent::Restored(_,_)|WindowEvent::Maximized(_,_)=>{},
                _=>{},
            }
        }
        let now=Instant::now();
        let real_delta=now.duration_since(last).as_secs_f32().clamp(0.0,0.1);
        last=now;
        accumulated+=(if game.double_speed {real_delta*2.0}else{real_delta}).min(0.12);
        // Fixed-timestep updates: enemy speed and tower DPS are stable at any FPS.
        let mut iterations=0;
        while accumulated>=1.0/60.0&&iterations<9 {
            game.update(1.0/60.0);accumulated-=1.0/60.0;iterations+=1;
        }
        if iterations==9 {accumulated=0.0;}
        let client=window.client_size();
        if let Some(viewport)=Viewport::new(client,(CANVAS_W as u32,CANVAS_H as u32)) {
            // Some window backends retain the last presented surface after
            // WM_SIZE. Resize the software surface *before* drawing anything.
            if (fb.width,fb.height)!=client {
                fb=Framebuffer::from_color(client.0,client.1,0xFF0B141F);
            } else {
                // Fully erase prior text, sprites, glows and alpha-blended
                // effects. Partial frame reuse produces trails/smearing.
                fb.clear(0xFF0B141F);
            }
            let mouse=raw_mouse.and_then(|(x,y)|viewport.pointer(x,y,(CANVAS_W as u32,CANVAS_H as u32)));
            {let mut p=Painter::new(&mut fb,&mut font,viewport);view::draw(&mut p,&game,mouse);}
            // WM_SIZE can occur while the CPU is rendering. Never ask the
            // native backend to stretch a stale framebuffer to a new size.
            if window.client_size()==client {window.present(&fb);}
        }
        // If minimized or the size changed mid-frame, skip presenting and
        // redraw a new native-resolution frame on the next iteration.
        // Limit unnecessary GPU/CPU use when idle; rendering is capped near 60 Hz.
        let next=previous_frame+Duration::from_micros(16_667);
        let after=Instant::now();
        if next>after {thread::sleep(next-after);}
        previous_frame=Instant::now();
    }
}

#[cfg(not(target_os="android"))]
fn main(){run(Window::new("AEGIS // Grid Defense",(1440,900)));}

#[cfg(target_os="android")]
#[unsafe(no_mangle)]
pub fn android_main(app:mata_win::AndroidApp){
    // MataWin mirrors Android primary-pointer taps to mouse events.
    // Packaging and signing must still be configured for Android deployment.
    run(Window::from_android_app(app,"AEGIS // Grid Defense",(1440,900)));
}

#[cfg(test)]
mod pointer_tests {
    use super::*;

    #[test]
    fn maps_mouse_to_same_logical_canvas_across_resizes() {
        for (w,h) in [(1440,900),(1920,1080),(900,600),(720,450),(1600,1200)] {
            let viewport=Viewport::new((w,h),(CANVAS_W as u32,CANVAS_H as u32)).unwrap();
            let cx=viewport.x+(viewport.width/2) as i32;
            let cy=viewport.y+(viewport.height/2) as i32;
            let (mx,my)=map_pointer(cx,cy,(w,h)).unwrap();
            // A physical pixel may cover more than one canvas pixel on shrink.
            assert!((mx-CANVAS_W/2).abs()<=2);
            assert!((my-CANVAS_H/2).abs()<=2);
            let (x0,y0)=map_pointer(viewport.x,viewport.y,(w,h)).unwrap();
            assert!(x0<=2 && y0<=2);
            assert!(map_pointer(viewport.x+viewport.width as i32,cy,(w,h)).is_none());
        }
    }

    #[test]
    fn clicking_a_rendered_tower_card_uses_the_same_viewport_as_drawing() {
        for (w,h) in [(1440,900),(1920,1080),(900,600),(1600,1200)] {
            let viewport=Viewport::new((w,h),(CANVAS_W as u32,CANVAS_H as u32)).unwrap();
            let x=viewport.x+(1220.0*viewport.width as f64/CANVAS_W as f64).round() as i32;
            let y=viewport.y+(251.0*viewport.height as f64/CANVAS_H as f64).round() as i32;
            let (bx,by)=map_pointer(x,y,(w,h)).unwrap();
            assert!(within(bx,by,1050,208,340,87));
        }
    }

    #[test]
    fn bars_outside_canvas_ignore_input() {
        // Wider and taller aspect ratios create noninteractive side/bottom bars.
        assert_eq!(map_pointer(0,500,(1920,1080)),None);
        assert_eq!(map_pointer(1000,0,(1600,1200)),None);
        assert_eq!(map_pointer(0,0,(0,900)),None);
        assert_eq!(map_pointer(0,0,(1440,0)),None);
        assert_eq!(map_pointer(-1,300,(1440,900)),None);
        assert_eq!(map_pointer(25,-1,(1440,900)),None);
    }

    #[test]
    fn viewport_stays_inside_native_client_and_preserves_aspect() {
        for (w,h) in [(1440,900),(1920,1080),(900,600),(1600,1200),(320,240),(3840,2160)] {
            let v=Viewport::new((w,h),(CANVAS_W as u32,CANVAS_H as u32)).unwrap();
            assert!(v.width<=w && v.height<=h);
            assert!(v.x>=0 && v.y>=0);
            assert!(v.x as u32+v.width<=w && v.y as u32+v.height<=h);
            let actual=v.width as f64/v.height as f64;
            let original=CANVAS_W as f64/CANVAS_H as f64;
            // One physical pixel of rounding may vary the ratio slightly.
            assert!((actual-original).abs() <= 2.0/v.height as f64);
        }
    }

    #[test]
    fn pointer_is_recalculated_after_resize_even_without_mouse_motion() {
        assert_eq!(map_pointer(500,300,(1440,900)),Some((500,300)));
        // 900x600 is narrower than 16:10, so it letterboxes vertically.
        assert_eq!(map_pointer(500,300,(900,600)),Some((800,451)));
    }
}
