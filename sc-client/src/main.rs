use std::collections::HashMap;
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

fn move_(unit: Unit, speed: f32) -> Vector2 {
    let dir_vec = HashMap::from([
        (Dir::Up, Vector2 { x: 0.0f32, y: -1.0f32 }),
        (Dir::Down, Vector2 { x: 0.0f32, y: 1.0f32 }),
        (Dir::Left, Vector2 { x: -1.0f32, y: 0.0f32 }),
        (Dir::Right, Vector2 { x: 1.0f32, y: 0.0f32 }),
        (Dir::Stop, Vector2::zero()),
    ]);
    let new_pos = unit.pos + dir_vec[&unit.dir].scale_by(speed);
    let unit_rect = Rect { x: new_pos.x.round() as i32, y: new_pos.y.round() as i32, w: 20, h: 20 };
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
        if safe_area && collide_rect(&r, &unit_rect) {
            is_safe = true;
        }
        if *t == AreaEnum::Blocked && collide_rect(&r, &unit_rect) {
            is_blocked = true;
        }
    }
    if !contain_rect(&play_area, &unit_rect) || (!is_safe && is_blocked) {
        unit.pos
    } else {
        new_pos
    }
}

fn apply_updates(units: &mut Vec<(UnitEnum, Unit)>, updates: &[GameCommand]) {
    for u in updates {
        match u {
            GameCommand::Move(u_id, d) => {
                let (t, u) = units[*u_id];
                units[*u_id] = (t, Unit { dir: *d, ..u });
            },
            GameCommand::Spawn(t, u) => {
                units.push((*t, *u));
            },
        }
    }
}

fn tab(game_state: &mut GameState) {
    game_state.selection = (game_state.selection + 1) % game_state.my_units.len();
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

fn collide_units(units: &Vec<(UnitEnum, Unit)>, p: Vector2, s: Vector2, unit_size: &HashMap<UnitEnum, Vector2>) -> bool {
    for (u_enum, u) in units {
        if collide_rect(&Rect { x: p.x, y: p.y, w: s.x, h: s.y }, &Rect { x: u.pos.x, y: u.pos.y, w: unit_size[&u_enum].x, h: unit_size[&u_enum].y}) {
            return true;
        }
    }
    false
}

fn spawn(game_state: &GameState, player_id: usize, t: UnitEnum, spawn_pos: &[Vector2; 2], unit_size: &HashMap<UnitEnum, Vector2>) -> Option<GameCommand> {
    if collide_units(&game_state.my_units, spawn_pos[player_id as usize], unit_size[&t], unit_size) ||
        collide_units(&game_state.other_units, spawn_pos[player_id as usize], unit_size[&t], unit_size) {
        None
    } else {
        Some(GameCommand::Spawn(t, Unit { player_id: player_id, pos: spawn_pos[player_id as usize], dir: Dir::Stop }))
    }
}

fn move_units(units: &mut Vec<(UnitEnum, Unit)>, unit_speeds: &HashMap<UnitEnum, f32>) {
    units.iter_mut().for_each(|unit| *unit = (unit.0, Unit { pos: move_(unit.1, unit_speeds[&unit.0]), ..unit.1 }));
}

fn main() -> std::io::Result<()> {
    let frame_rate = 60;
    let max_input_queue = 10;
    let unit_speeds = HashMap::from([
        (UnitEnum::Interceptor, 1.0f32),
        (UnitEnum::MessageBox, 1.0f32)
    ]);
    let unit_size = HashMap::from([
        (UnitEnum::Interceptor, Vector2 { x: 20.0, y: 20.0 }),
        (UnitEnum::MessageBox, Vector2 { x: 20.0, y: 20.0 })
    ]);
    let p0_colors = HashMap::from([
        (UnitEnum::Interceptor, Color::from_hex("90E0EF").unwrap()),
        (UnitEnum::MessageBox, Color::from_hex("90E0EF").unwrap()),
    ]);
    let p1_colors = HashMap::from([
        (UnitEnum::Interceptor, Color::from_hex("74C69D").unwrap()),
        (UnitEnum::MessageBox, Color::from_hex("74C69D").unwrap()),
    ]);
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
    if args.len() < 2 {
        println!("Usage {} server_addr", args[0]);
        std::process::exit(1);
    }
    let server_addr = &args[1][..];

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
    let mut game_state: GameState = GameState { my_units: vec![], other_units: vec![], selection: 0 };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    let mut s_time = 0f64;
    let mut sent_frame = 0;
    let mut unsent_pkt = vec![];
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mut go = false;

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
                            apply_updates(&mut game_state.other_units, &updates);
                            go = true;
                        },
                        Some(_) => {
                            panic!("Expected UpdateOtherTarget")
                        }
                    }
                }

                loop {
                    if unsent_pkt.len() > max_input_queue {
                        break;
                    }

                    match rl.get_key_pressed() {
                        Some(k) => {
                            match k {
                                KeyboardKey::KEY_W => unsent_pkt.push(GameCommand::Move(game_state.selection, Dir::Up)),
                                KeyboardKey::KEY_A => unsent_pkt.push(GameCommand::Move(game_state.selection, Dir::Left)),
                                KeyboardKey::KEY_S => unsent_pkt.push(GameCommand::Move(game_state.selection, Dir::Down)),
                                KeyboardKey::KEY_D => unsent_pkt.push(GameCommand::Move(game_state.selection, Dir::Right)),
                                KeyboardKey::KEY_H => unsent_pkt.push(GameCommand::Move(game_state.selection, Dir::Stop)),
                                KeyboardKey::KEY_M => { spawn(&game_state, p_id, UnitEnum::MessageBox, &msg_spawn_pos, &unit_size).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_I => { spawn(&game_state, p_id, UnitEnum::Interceptor, &msg_spawn_pos, &unit_size).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_O => { spawn(&game_state, p_id, UnitEnum::Interceptor, &int_spawn_pos, &unit_size).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_TAB => tab(&mut game_state),
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

                    apply_updates(&mut game_state.my_units, &unsent_pkt);
                    unsent_pkt = vec![];
                    sent_frame += 2;
                }

                if (go || (frame_counter % 2 == 1)) && !ended {
                    move_units(&mut game_state.my_units, &unit_speeds);
                    move_units(&mut game_state.other_units, &unit_speeds);
                    frame_counter += 1;
                }

                // todo!("Collision Detection") and out of arena detection
                ClientState::Started(false)
            },
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);

        for (t, r) in &GAME_MAP {
            match t {
                AreaEnum::P0Station => d.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&t]),
                AreaEnum::P1Station => d.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&t]),
                AreaEnum::Blocked => d.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&t]),
                _ => d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]),
            }
        }

        for (i, (t, u)) in game_state.my_units.iter().chain(game_state.other_units.iter()).enumerate() {
            let c = if u.player_id == 0 { p0_colors[&t] } else { p1_colors[&t] };
            match t {
                UnitEnum::Interceptor => {
                    let cen = u.pos + unit_size[&t].scale_by(0.5f32);
                    d.draw_circle(cen.x.round() as i32, cen.y.round() as i32, unit_size[&t].x/2f32, c);
                },
                UnitEnum::MessageBox => d.draw_rectangle_v(u.pos, unit_size[&t], c),
            }
            if game_state.selection < game_state.my_units.len() && game_state.selection == i {
                match t {
                    UnitEnum::Interceptor => {
                        let cen = u.pos + unit_size[&t].scale_by(0.5f32);
                        d.draw_circle_lines(cen.x.round() as i32, cen.y.round() as i32, unit_size[&t].x/2f32, Color::BLACK);
                    },
                    UnitEnum::MessageBox => {
                        d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, unit_size[&t].x.round() as i32, unit_size[&t].y.round() as i32, Color::BLACK)
                    },
                }
            }
        }

        d.draw_text(&state.to_string(), 20, 20, 20, Color::BLACK);
        d.draw_text(&frame_counter.to_string(), 20, 40, 20, Color::BLACK);

    }
    Ok(())
}