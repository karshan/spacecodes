use std::collections::HashMap;
use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use std::fmt;
use raylib::prelude::*;
use sc_types::*;

mod util;
use util::*;

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

fn move_(pos: Vector2, dir: Dir, speed: f32) -> Vector2 {
    let dir_vec = HashMap::from([
        (Dir::Up, Vector2 { x: 0.0f32, y: -1.0f32 }),
        (Dir::Down, Vector2 { x: 0.0f32, y: 1.0f32 }),
        (Dir::Left, Vector2 { x: -1.0f32, y: 0.0f32 }),
        (Dir::Right, Vector2 { x: 1.0f32, y: 0.0f32 }),
        (Dir::Stop, Vector2::zero()),
    ]);
    pos + dir_vec[&dir].scale_by(speed)
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

fn collides(units: &Vec<(UnitEnum, Unit)>, p: Vector2, s: Vector2, unit_size: &HashMap<UnitEnum, Vector2>) -> bool {
    for (u_enum, u) in units {
        let t = p.y;
        let b = p.y + s.y;
        let l = p.x;
        let r = p.x + s.x;
        let tt = u.pos.y;
        let bb = u.pos.y + unit_size[&u_enum].y;
        let ll = u.pos.x;
        let rr = u.pos.x + unit_size[&u_enum].x;
        if !(b < tt || t > bb || l > rr || r < ll) {
            return true;
        }
    }
    false
}

fn spawn(game_state: &GameState, player_id: u8, t: UnitEnum, spawn_pos: &[Vector2; 2], unit_size: &HashMap<UnitEnum, Vector2>) -> Option<GameCommand> {
    if collides(&game_state.my_units, spawn_pos[player_id as usize], unit_size[&t], unit_size) ||
        collides(&game_state.other_units, spawn_pos[player_id as usize], unit_size[&t], unit_size) {
        None
    } else {
        Some(GameCommand::Spawn(t, Unit { player_id: player_id, pos: spawn_pos[player_id as usize], dir: Dir::Stop }))
    }
}

fn move_units(units: &mut Vec<(UnitEnum, Unit)>, unit_speeds: &HashMap<UnitEnum, f32>) {
    units.iter_mut().for_each(|unit| *unit = (unit.0, Unit { pos: move_(unit.1.pos, unit.1.dir, unit_speeds[&unit.0]), ..unit.1 }));
}

fn main() -> std::io::Result<()> {
    let frame_rate = 15;
    let max_input_queue = 10;
    let unit_speeds = HashMap::from([
        (UnitEnum::Interceptor, 1.0f32),
        (UnitEnum::MessageBox, 1.0f32)
    ]);
    let unit_size = HashMap::from([
        (UnitEnum::Interceptor, Vector2 { x: 10.0, y: 10.0 }),
        (UnitEnum::MessageBox, Vector2 { x: 10.0, y: 10.0 })
    ]);
    let player_colors = HashMap::from([
        (UnitEnum::Interceptor, Color::DARKPURPLE),
        (UnitEnum::MessageBox, Color::BLUE),
    ]);
    let enemy_colors = HashMap::from([
        (UnitEnum::Interceptor, Color::ORANGE),
        (UnitEnum::MessageBox, Color::RED),
    ]);
    let spawn_pos = [Vector2 { x: 0.0f32, y: 0.0f32 }, Vector2 { x: 600.0f32, y: 440.0f32 }];


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
        .size(640, 480)
        .title("Space Codes")
        .build();
    rl.set_target_fps(frame_rate);

    let mut state = ClientState::SendHello;
    let mut game_state: GameState = GameState { my_units: vec![], other_units: vec![], selection: 0 };
    let mut p_id = 0u8;
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
                                KeyboardKey::KEY_M => { spawn(&game_state, p_id, UnitEnum::MessageBox, &spawn_pos, &unit_size).map(|c| unsent_pkt.push(c)); },
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

        for (i, (t, u)) in game_state.my_units.iter().enumerate() {
            d.draw_rectangle_v(u.pos, unit_size[&t], player_colors[&t]);
            if game_state.selection == i {
                d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, unit_size[&t].x.round() as i32 + 1, unit_size[&t].y.round() as i32 + 1, Color::BLACK)
            }
        }

        for (t, u) in game_state.other_units.iter() {
            d.draw_rectangle_v(u.pos, unit_size[&t], enemy_colors[&t]);
        }

        d.draw_text(&state.to_string(), 20, 20, 20, Color::BLACK);
        d.draw_text(&frame_counter.to_string(), 20, 40, 20, Color::BLACK);

    }
    Ok(())
}