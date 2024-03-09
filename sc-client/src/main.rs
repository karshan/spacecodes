use std::collections::HashMap;
use std::cmp::{min, max};
use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use std::fmt;
use num_traits::Num;
use raylib::prelude::*;
use sc_types::*;

mod util;
use util::*;

struct Rect<T: Num> {
    x: T,
    y: T,
    w: T,
    h: T,
}

#[derive(Eq, PartialEq, Hash)]
enum AreaEnum {
    P0Spawn,
    P1Spawn,
    P0Station,
    P1Station,
    Blocked
}

static START_FUEL: i32 = 3600;
static FUEL_LOSS: i32 = 1; // per frame
static MSG_FUEL: i32 = 600;
static INTERCEPT_RADIUS: f32 = 40f32;
static INTERCEPTOR_EXPIRY: i32 = 1800;
static GAME_MAP: [(AreaEnum, Rect<i32>); 5] = [
    (AreaEnum::Blocked, Rect {
        x: 328, y: 200,
        w: 368, h: 368
    }),
    (AreaEnum::P0Spawn, Rect {
        x: 477, y: 200,
        w: 70, h: 70
    }),
    (AreaEnum::P0Station, Rect {
        x: 477, y: 498,
        w: 70, h: 70
    }),
    (AreaEnum::P1Spawn, Rect {
        x: 626, y: 349,
        w: 70, h: 70
    }),
    (AreaEnum::P1Station, Rect {
        x: 328, y: 349,
        w: 70, h: 70
    }),
];

enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started(bool),
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClientState::SendHello => write!(f, "SendHello"),
            ClientState::ExpectWelcome => write!(f, "ExpectWelcome"),
            ClientState::Waiting => write!(f, "Waiting"),
            ClientState::Started(b) => write!(f, "Started {}", b),
        }
    }
}

fn unit_rect(t: UnitEnum, u: Unit) -> Rect<i32> {
    Rect { x: u.pos.x.round() as i32, y: u.pos.y.round() as i32, w: t.size().x.round() as i32, h: t.size().y.round() as i32 }
}

fn move_(unit: Unit, speed: f32) -> Vector2 {
    let new_pos = if (unit.target - unit.pos).length_sqr() < speed * speed {
            unit.target
        } else {
            unit.pos + (unit.target - unit.pos).normalized().scale_by(speed)
        };
    let ur = Rect { x: new_pos.x.round() as i32, y: new_pos.y.round() as i32, w: 20, h: 20 };
    let play_area = Rect { x: 0, y: 0, w: 1024, h: 768 };
    let mut is_safe = false;
    let mut is_blocked = false;
    for (t, r) in &GAME_MAP {
        let safe_area = match t {
            AreaEnum::P0Spawn => unit.player_id == 0,
            AreaEnum::P0Station => unit.player_id == 0,
            AreaEnum::P1Spawn => unit.player_id == 1,
            AreaEnum::P1Station => unit.player_id == 1,
            _ => false
        };
        if safe_area && collide_rect(&r, &ur) {
            is_safe = true;
        }
        if *t == AreaEnum::Blocked && collide_rect(&r, &ur) {
            is_blocked = true;
        }
    }
    if !contain_rect(&play_area, &ur) || (!is_safe && is_blocked) {
        unit.pos
    } else {
        new_pos
    }
}

fn apply_updates(units: &mut Vec<(UnitEnum, Unit)>, updates: &[GameCommand], other_units: &mut Vec<(UnitEnum, Unit)>, animations: &mut Vec<Vector2>) {
    for u in updates {
        match u {
            GameCommand::Move(MoveCommand { u_id, target }) => {
                let (t, u) = units[*u_id];
                units[*u_id] = (t, Unit { target: *target, ..u });
            },
            GameCommand::Spawn(t, u) => {
                units.push((*t, *u));
            },
            GameCommand::Intercept(InterceptCommand { u_id, pos }) => {
                units[*u_id].1.cooldown = UnitEnum::Interceptor(0).cooldown();
                animations.push(pos.clone());
                for (t, unit) in other_units.iter_mut() {
                    match t {
                        UnitEnum::MessageBox => {
                            let unit_cen = unit.pos + t.size().scale_by(0.5f32);
                            if (unit_cen.x - pos.x).powf(2f32) + (unit_cen.y - pos.y).powf(2f32) <= INTERCEPT_RADIUS.powf(2f32) {
                                *t = UnitEnum::Dead;
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}

fn add_fuel(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let f = |(t, u): &(UnitEnum, Unit)| match t {
        // FIXME need to be able to lookup ship/station rects
        UnitEnum::MessageBox => !collide_rect(&unit_rect(*t, *u), &GAME_MAP[2 + u.player_id * 2].1),
        _ => true
    };

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    reap(game_state);
    game_state.other_units.retain(f);


    game_state.fuel[p_id] = min(START_FUEL, game_state.fuel[p_id] + (num_my_units - game_state.my_units.len() as i32) * MSG_FUEL);
    game_state.fuel[other_id] = min(START_FUEL, game_state.fuel[other_id] + (num_other_units - game_state.other_units.len() as i32) * MSG_FUEL);
}

fn tick_cd_expiry(game_state: &mut GameState) {
    for (t, u) in game_state.my_units.iter_mut().chain(game_state.other_units.iter_mut()) {
        u.cooldown = max(0, u.cooldown - 1);
        if let UnitEnum::Interceptor(e) = t {
            if *e == 1 {
                *t = UnitEnum::Dead
            } else {
                *t = UnitEnum::Interceptor(max(0, *e - 1))
            }
        }
    }

    reap(game_state);
    game_state.other_units.retain(|(t, _)| match t {
        UnitEnum::Dead => false,
        _ => true
    });
    
}

fn contain_rect<T: Num + PartialOrd + Copy>(parent: &Rect<T>, child: &Rect<T>) -> bool {
    child.x >= parent.x && child.x + child.w <= parent.x + parent.w &&
        child.y >= parent.y && child.y + child.h <= parent.y + parent.h
}

fn collide_rect<T: Num + PartialOrd + Copy>(r1: &Rect<T>, r2: &Rect<T>) -> bool {
    let t = r1.y;
    let b = t + r1.h;
    let l = r1.x;
    let r = l + r1.w;
    let tt = r2.y;
    let bb = tt + r2.h;
    let ll = r2.x;
    let rr = ll + r2.w;
    !(b < tt || t > bb || l > rr || r < ll) 
}

fn collide_units(units: &Vec<(UnitEnum, Unit)>, p: &Vector2, s: &Vector2) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];
    for (i, (u_enum, u)) in units.iter().enumerate() {
        if collide_rect(&Rect { x: p.x, y: p.y, w: s.x, h: s.y }, &Rect { x: u.pos.x, y: u.pos.y, w: u_enum.size().x, h: u_enum.size().y}) {
            out.push(i);
        }
    }
    out
}

fn spawn(game_state: &GameState, player_id: usize, t: UnitEnum, spawn_pos: &[Vector2; 2]) -> Option<GameCommand> {
    if !collide_units(&game_state.my_units, &spawn_pos[player_id as usize], t.size()).is_empty() ||
        !collide_units(&game_state.other_units, &spawn_pos[player_id as usize], t.size()).is_empty() {
        None
    } else {
        Some(GameCommand::Spawn(t, Unit { player_id: player_id, pos: spawn_pos[player_id as usize], target: spawn_pos[player_id as usize], cooldown: 0 }))
    }
}

fn selected_units(game_state: &GameState) -> Vec<(usize, (UnitEnum, Unit))> {
    let mut out = vec![];
    for s in &game_state.selection {
        if let Selection::Unit(u_id) = s {
            if *u_id < game_state.my_units.len() {
                out.push((*u_id, game_state.my_units[*u_id]))
            }
        }
    }
    out
}

fn move_units(units: &mut Vec<(UnitEnum, Unit)>) {
    units.iter_mut().for_each(|unit| *unit = (unit.0, Unit { pos: move_(unit.1, unit.0.speed()), ..unit.1 }));
}

fn reap(game_state: &mut GameState) {
    let mut out = vec![];
    for s in &game_state.selection {
        if let Selection::Unit(selection_uid) = s {
            match game_state.my_units[*selection_uid] {
                (UnitEnum::Dead, _) => {}
                _ => {
                    let mut count_dead = 0;
                    for i in 0..*selection_uid {
                        if let (UnitEnum::Dead, _) = game_state.my_units[i] {
                            count_dead += 1;
                        }
                    }
                    out.push(Selection::Unit(*selection_uid - count_dead));
                }
            }
        } else {
            out.push(*s)
        }
    }
    game_state.selection = out;

    game_state.my_units.retain(|(t, _)| match t {
        UnitEnum::Dead => false,
        _ => true
    });
}

fn main() -> std::io::Result<()> {
    let frame_rate = 60;
    let max_input_queue = 10;
        let area_colors = HashMap::from([
        (AreaEnum::P0Spawn, Color::from_hex("0077B6").unwrap()),
        (AreaEnum::P0Station, Color::from_hex("0077B6").unwrap()),
        (AreaEnum::P1Spawn, Color::from_hex("1B4332").unwrap()),
        (AreaEnum::P1Station, Color::from_hex("1B4332").unwrap()),
        (AreaEnum::Blocked, Color::from_hex("D8F3DC").unwrap()),
    ]);
    let msg_spawn_pos = [Vector2 { x: 502f32, y: 250f32 }, Vector2 { x: 626f32, y: 374f32 }];
    let int_spawn_pos = [Vector2 { x: 502f32, y: 498f32 }, Vector2 { x: 378f32, y: 374f32 }];


    let args: Vec<String> = env::args().collect();
    let mut server_addr = "192.168.1.145:8080";
    if args.len() >= 2 {
        //println!("Usage {} server_addr", args[0]);
        //std::process::exit(1);
        server_addr = &args[1][..];
    }

    let server: Vec<std::net::SocketAddr> = server_addr
        .to_socket_addrs()
        .expect("Unable to resolve domain")
        .collect();
    if server.len() < 1 {
        panic!("unable to resolve server?")
    }

    set_trace_log(TraceLogLevel::LOG_ERROR);
    let (mut rl, thread) = raylib::init()
        .size(1024, 768)
        .title("Space Codes")
        .build();
    rl.set_target_fps(frame_rate);

    let mut state = ClientState::SendHello;
    let mut game_state: GameState = GameState { my_units: vec![], other_units: vec![], selection: vec![], fuel: [START_FUEL; 2] };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    let mut s_time = 0f64;
    let mut sent_frame = 0;
    let mut unsent_pkt = vec![];
    let mut animations = vec![];
    let mut drag_select: Option<Vector2> = None;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mut go = false;
        let mouse_position = rl.get_mouse_position();

        state = match state {
            ClientState::SendHello => {
                socket_send(&socket, &server[0], &ClientPkt::Hello { seq: seq_state.send_seq, sent_time: rl.get_time() })?;
                seq_state.send();
                ClientState::ExpectWelcome
            },
            ClientState::ExpectWelcome => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => ClientState::ExpectWelcome,
                    Some(ServerEnum::Welcome { handshake_start_time, player_id }) => {
                        p_id = player_id;
                        ClientState::Waiting
                    },
                    Some(_) => {
                        panic!("Expected Welcome")
                    },
                }
            },
            ClientState::Waiting => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => ClientState::Waiting,
                    Some(ServerEnum::Start) => {
                        frame_counter = 0;
                        unsent_pkt = vec![];
                        ClientState::Started(false)
                    },
                    Some(_) => {
                        panic!("Expected Start")
                    }
                }
            },
            ClientState::Started(ended) => {
                if frame_counter % 2 == 0 {
                    let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                    match resp {
                        None => {},
                        Some(ServerEnum::UpdateOtherTarget { updates, frame }) => {
                            apply_updates(&mut game_state.other_units, &updates, &mut game_state.my_units, &mut animations);
                            reap(&mut game_state);
                            go = true;
                        },
                        Some(_) => {
                            panic!("Expected UpdateOtherTarget")
                        }
                    }
                }

                drag_select = match drag_select {
                    None => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            Some(mouse_position)
                        } else {
                            None
                        }
                    },
                    Some(start_pos) => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            Some(start_pos)
                        } else {
                            let selection_pos = Vector2 { x: start_pos.x.min(mouse_position.x), y: start_pos.y.min(mouse_position.y) };
                            let selection_size = Vector2 { x: (start_pos.x - mouse_position.x).abs(), y: (start_pos.y - mouse_position.y).abs() };
                            game_state.selection = vec![];
                            game_state.selection.append(&mut collide_units(&game_state.my_units, &selection_pos, &selection_size).iter().map(|u_id| Selection::Unit(*u_id)).collect());
                            None
                        }
                    }
                };

                if rl.is_mouse_button_pressed(MouseButton::MOUSE_RIGHT_BUTTON) && unsent_pkt.len() < max_input_queue {
                    for (u_id, (t, _)) in selected_units(&game_state) {
                        unsent_pkt.push(GameCommand::Move(MoveCommand { u_id: u_id, target: rl.get_mouse_position() - t.size().scale_by(0.5f32) }));
                    }
                }

                loop {
                    if unsent_pkt.len() >= max_input_queue {
                        break;
                    }

                    match rl.get_key_pressed() {
                        Some(k) => {
                            match k {
                                KeyboardKey::KEY_H => { 
                                    for (u_id, (_, u)) in selected_units(&game_state) { 
                                        unsent_pkt.push(GameCommand::Move(MoveCommand { u_id: u_id, target: u.pos })); 
                                    }
                                },
                                KeyboardKey::KEY_M => { spawn(&game_state, p_id, UnitEnum::MessageBox, &msg_spawn_pos).map(|c| unsent_pkt.push(c)); }
                                KeyboardKey::KEY_I => { spawn(&game_state, p_id, UnitEnum::Interceptor(INTERCEPTOR_EXPIRY), &msg_spawn_pos).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_O => { spawn(&game_state, p_id, UnitEnum::Interceptor(INTERCEPTOR_EXPIRY), &int_spawn_pos).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_ONE => game_state.selection = vec![Selection::Ship],
                                KeyboardKey::KEY_TWO => game_state.selection = vec![Selection::Station],
                                KeyboardKey::KEY_SPACE => {
                                    for (u_id, (t, u)) in selected_units(&game_state) {
                                        if let UnitEnum::Interceptor(_) = t {
                                            if u.cooldown <= 0 {
                                                unsent_pkt.push(GameCommand::Intercept(InterceptCommand { u_id: u_id, pos: u.pos + UnitEnum::Interceptor(0).size().scale_by(0.5f32) }));
                                            }
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                        None => break
                    }
                }

                if sent_frame <= frame_counter && (frame_counter % 2 == 0) {
                    socket_send(&socket, &server[0], &ClientPkt::Target { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        updates: unsent_pkt.clone(),
                        frame: frame_counter,
                    })?;
                    seq_state.send();

                    apply_updates(&mut game_state.my_units, &unsent_pkt, &mut game_state.other_units, &mut animations);
                    unsent_pkt = vec![];
                    sent_frame += 2;
                }

                if (go || (frame_counter % 2 == 1)) && !ended {
                    move_units(&mut game_state.my_units);
                    move_units(&mut game_state.other_units);
                    add_fuel(&mut game_state, p_id);
                    game_state.fuel.iter_mut().for_each(|f| *f -= FUEL_LOSS);
                    tick_cd_expiry(&mut game_state);
                    frame_counter += 1;
                }

                if game_state.fuel.iter().any(|f| *f < 0) {
                    ClientState::Started(true)
                } else {
                    ClientState::Started(false)
                }
            },
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);

        for (t, r) in &GAME_MAP {
            match t {
                AreaEnum::P0Station => d.draw_rectangle(r.x, r.y, r.w, (r.h * game_state.fuel[0])/START_FUEL, area_colors[&t]),
                AreaEnum::P1Station => d.draw_rectangle(r.x + (r.w * (START_FUEL - game_state.fuel[1]))/START_FUEL, r.y, (r.w * game_state.fuel[1])/START_FUEL, r.h, area_colors[&t]),
                AreaEnum::Blocked => d.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&t]),
                _ => d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]),
            }
        }

        for (i, (t, u)) in game_state.my_units.iter().chain(game_state.other_units.iter()).enumerate() {
            let c = if u.player_id == 0 { t.p0_colors() } else { t.p1_colors() };
            match t {
                UnitEnum::Interceptor(_) => {
                    let cen = u.pos + t.size().scale_by(0.5f32);
                    d.draw_circle(cen.x.round() as i32, cen.y.round() as i32, t.size().x/2f32, c);
                },
                UnitEnum::MessageBox => d.draw_rectangle_v(u.pos, t.size(), c),
                _ => {}
            }
            for s in &game_state.selection {
                match s {
                    Selection::Unit(u_id) if *u_id == i => {
                        match t {
                            UnitEnum::Interceptor(e) => {
                                let cen = u.pos + t.size().scale_by(0.5f32);
                                d.draw_circle_lines(cen.x.round() as i32, cen.y.round() as i32, t.size().x/2f32, Color::BLACK);
                                d.draw_text(&format!("E: {}", e), 20, 80, 20, Color::BLACK);

                            },
                            UnitEnum::MessageBox => {
                                d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, t.size().x.round() as i32, t.size().y.round() as i32, Color::BLACK)
                            },
                            _ => {}
                        }
                        d.draw_text(&format!("CD: {}", u.cooldown), 20, 60, 20, Color::BLACK);
                    },
                    Selection::Ship => {
                        // FIXME need to be able to lookup ship/station rects
                        let rect = &GAME_MAP[1 + p_id*2].1;
                        d.draw_rectangle_lines(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2, Color::BLACK)
                    },
                    Selection::Station => {
                        // FIXME need to be able to lookup ship/station rects
                        let rect = &GAME_MAP[2 + p_id*2].1;
                        d.draw_rectangle_lines(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2, Color::BLACK)
                    },
                    Selection::Unit(_) => {}
                }
            }
        }

        for a in animations {
            d.draw_circle(a.x.round() as i32, a.y.round() as i32, INTERCEPT_RADIUS - 10f32, Color::BLACK);
        }
        animations = vec![];

        if let Some(start_pos) = drag_select {
            let selection_pos = Vector2 { x: start_pos.x.min(mouse_position.x), y: start_pos.y.min(mouse_position.y) };
            let selection_size = Vector2 { x: (start_pos.x - mouse_position.x).abs(), y: (start_pos.y - mouse_position.y).abs() };
            d.draw_rectangle_lines(selection_pos.x as i32, selection_pos.y as i32, selection_size.x as i32, selection_size.y as i32, Color::GREEN)
        }

        d.draw_text(&state.to_string(), 20, 20, 20, Color::BLACK);
        d.draw_text(&frame_counter.to_string(), 20, 40, 20, Color::BLACK);


    }
    Ok(())
}