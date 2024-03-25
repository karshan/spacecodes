use std::collections::{HashMap, HashSet, VecDeque};
use std::cmp::{min, max};
use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use pathfinding::path_collides;
use raylib::prelude::*;
use sc_types::*;
use sc_types::shapes::*;
extern crate rmp_serde as rmps;

mod util;
mod pathfinding;
mod types;

use util::*;
use sc_types::constants::*;
use types::*;

fn move_unit(unit: &mut Unit) -> () {
    // TODO blink around turns
    // FIXME don't slow down on turns
    if !unit.path.is_empty() {
        let speed = unit.speed();
        unit.pos = if unit.blinking {
            if (unit.path[0] - unit.pos).length() < BLINK_RANGE {
                unit.path[0]
            } else {
                unit.pos + (unit.path[0] - unit.pos).normalized().scale_by(BLINK_RANGE)
            }
        } else {
            if (unit.path[0] - unit.pos).length() < speed {
                unit.path[0]
            } else {
                unit.pos + (unit.path[0] - unit.pos).normalized().scale_by(speed)
            }
        };

        if unit.pos == unit.path[0] {
            unit.path.pop_front();
        }
    }
    unit.blinking = false;
}

fn apply_updates(intercepted_count: &mut u8, units: &mut Vec<Unit>, updates: &[GameCommand], other_units: &mut Vec<Unit>, animations: &mut Vec<Vector2>) {
    for u in updates {
        match u {
            GameCommand::Blink(BlinkCommand { u_id }) => {
                if *u_id < units.len() {
                    units[*u_id].cooldown = units[*u_id].cooldown();
                    units[*u_id].blinking = true;
                }
            },
            GameCommand::Spawn(SpawnMsgCommand { path, player_id }) => {
                units.push(Unit { dead: false, player_id: *player_id, pos: path[0], path: path.clone(), blinking: false, cooldown: 0 });
            },
            GameCommand::Intercept(InterceptCommand { pos }) => {
                animations.push(pos.clone());
                for unit in other_units.iter_mut() {
                    if !unit.dead {
                        let unit_cen = unit.pos + unit.size().scale_by(0.5f32);
                        if (unit_cen.x - pos.x).powf(2f32) + (unit_cen.y - pos.y).powf(2f32) <= INTERCEPT_RADIUS.powf(2f32) {
                            unit.dead = true;
                            *intercepted_count += 1;
                        }
                    }
                }
                todo!("Intercept cooldown");
            }
        }
    }
}

fn add_fuel(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    game_state.my_units.iter_mut().filter(|u| !u.dead && u.rect().collide(station(u.player_id)))
        .for_each(|u| u.dead = true);
    reap(game_state);
    game_state.other_units.retain(|u| !u.rect().collide(station(u.player_id)));

    game_state.fuel[p_id] = min(START_FUEL, game_state.fuel[p_id] + (num_my_units - game_state.my_units.len() as i32) * MSG_FUEL);
    game_state.fuel[other_id] = min(START_FUEL, game_state.fuel[other_id] + (num_other_units - game_state.other_units.len() as i32) * MSG_FUEL);
}

fn tick_cd_expiry(game_state: &mut GameState) {
    for u in game_state.my_units.iter_mut().chain(game_state.other_units.iter_mut()) {
        u.cooldown = max(0, u.cooldown - 1);
    }
}

fn collide_units(units: &Vec<Unit>, p: &Vector2, s: &Vector2) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];
    for (i, u) in units.iter().enumerate() {
        if (Rect { x: p.x, y: p.y, w: s.x, h: s.y }).collide(&Rect { x: u.pos.x, y: u.pos.y, w: u.size().x, h: u.size().y }) {
            out.push(i);
        }
    }
    out
}

fn selected_units(game_state: &GameState) -> Vec<(usize, Unit)> {
    let mut out = vec![];
    for s in &game_state.selection {
        if let Selection::Unit(u_id) = s {
            if *u_id < game_state.my_units.len() {
                out.push((*u_id, game_state.my_units[*u_id].clone()))
            }
        }
    }
    out
}

fn reap(game_state: &mut GameState) {
    let mut out = HashSet::new();
    for s in &game_state.selection {
        if let Selection::Unit(selection_uid) = s {
            if !game_state.my_units[*selection_uid].dead {
                let mut count_dead = 0;
                for i in 0..*selection_uid {
                    if game_state.my_units[i].dead {
                        count_dead += 1;
                    }
                }
                out.insert(Selection::Unit(*selection_uid - count_dead));
            }
        } else {
            out.insert(*s);
        }
    }
    game_state.selection = out;
    game_state.my_units.retain(|u| !u.dead);
}

fn serialize_state(game_state: &GameState, p_id: usize) -> Result<Vec<u8>, rmps::encode::Error> {
    let mut v;
    if p_id == 0 {
        v = rmp_serde::encode::to_vec(&game_state.my_units)?;
        v.append(&mut rmp_serde::encode::to_vec(&game_state.other_units)?);
    } else {
        v = rmp_serde::encode::to_vec(&game_state.other_units)?;
        v.append(&mut rmp_serde::encode::to_vec(&game_state.my_units)?);
    }
    v.append(&mut rmp_serde::encode::to_vec(&game_state.fuel)?);
    v.append(&mut rmp_serde::encode::to_vec(&game_state.intercepted)?);
    Ok(v)
}

fn get_manhattan_turn_point(p1: Vector2, p2: Vector2, p_id: usize) -> Option<Vector2> {
    let m1 = Vector2 { x: p1.x, y: p2.y };
    let m2 = Vector2 { x: p2.x, y: p1.y };
    let sx = Vector2 { x: MESSAGE_SIZE.x, y: 0f32 };
    let sy = Vector2 { x: 0f32, y: MESSAGE_SIZE.y };
    let offsets = [ Vector2::zero(), sx, sy, *MESSAGE_SIZE ];
    let blocked = if p_id == 0 { &P0_BLOCKED } else { &P1_BLOCKED };
    let m1_ok = !path_collides(blocked, offsets, p1, m1) && !path_collides(blocked, offsets, m1, p2);
    let m2_ok = !path_collides(blocked, offsets, p1, m2) && !path_collides(blocked, offsets, m2, p2);
    if m1_ok && m2_ok {
        if (p1.x - p2.x).abs() < (p1.y - p2.y).abs() {
            Some(m1)
        } else {
            Some(m2)
        }
    } else if m1_ok {
        Some(m1)
    } else if m2_ok {
        Some(m2)
    } else {
        None
    }
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
    let msg_spawn_pos = [Vector2 { x: 502f32, y: 249f32 }, Vector2 { x: 627f32, y: 374f32 }];

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
    // Most of these values doesn't matter. Its just for the compiler. They are initialized in ClientState::Waiting
    let mut game_state: GameState = GameState { my_units: vec![], other_units: vec![], selection: HashSet::new(), fuel: [START_FUEL; 2], intercepted: [0; 2] };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    let mut sent_frame = 0;
    let mut frame_state = FrameState::Neither;
    // TODO All this netcode related stuff should be abstracted into a single type
    let mut unsent_pkt = vec![];
    let mut unacked_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut future_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut sent_pkt = vec![];
    let mut recvd_pkt = vec![];
    let mut last_rcvd_pkt = -1;
    // ------------------------
    let mut animations = vec![];
    enum MouseState {
        Drag(Vector2),
        Path(VecDeque<Vector2>),
        None
    }
    let mut mouse_state: MouseState = MouseState::None;
    let mut ended = None;
    let mut packets_ps = WindowAvg::new();
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mouse_position = rl.get_mouse_position();
        let fps = rl.get_fps();

        state = match state {
            ClientState::SendHello => {
                ended = None;
                socket_send(&socket, &server[0], &ClientPkt::Hello { seq: seq_state.send_seq, sent_time: rl.get_time() })?;
                seq_state.send();
                ClientState::ExpectWelcome
            },
            ClientState::ExpectWelcome => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state);
                match resp {
                    None => ClientState::ExpectWelcome,
                    Some(ServerEnum::Welcome { handshake_start_time: _, player_id }) => {
                        p_id = player_id;
                        ClientState::Waiting
                    },
                    Some(_) => {
                        panic!("Expected Welcome")
                    },
                }
            },
            ClientState::Waiting => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state);
                match resp {
                    None => ClientState::Waiting,
                    Some(ServerEnum::Start) => {
                        frame_counter = 0;
                        sent_frame = 0;
                        unsent_pkt = vec![];
                        unacked_pkts = VecDeque::new();
                        future_pkts = VecDeque::new();
                        last_rcvd_pkt = -1;
                        ended = None;
                        animations = vec![];
                        mouse_state = MouseState::None;
                        frame_state = FrameState::Neither;
                        game_state = GameState { my_units: vec![], other_units: vec![], selection: HashSet::new(), fuel: [START_FUEL; 2], intercepted: [0; 2] };
                        ClientState::Started
                    },
                    Some(_) => {
                        panic!("Expected Start")
                    }
                }
            },
            ClientState::Started => {
                // frame_state can never be Both here. It is reset to neither if it is Both
                if frame_counter % 2 == 0 && frame_state != FrameState::Received {
                    let resp = socket_recv(&socket, &server[0], &mut seq_state);
                    match resp {
                        None => {
                            match future_pkts.iter().find(|ps| ps.0 == frame_counter) {
                                Some(p) => {
                                    recvd_pkt = p.1.clone();
                                    future_pkts.retain(|ps| ps.0 > frame_counter);
                                    frame_state.recvd();
                                },
                                None => {}
                            };
                        },
                        Some(ServerEnum::UpdateOtherTarget { updates, frame, frame_ack }) => {
                            if frame != frame_counter {
                                println!("out of order packet {}", frame - frame_counter);
                            }
                            frame_state.recvd();
                            recvd_pkt = updates.iter().chain(future_pkts.iter())
                                .find(|ps| ps.0 == frame_counter).expect("recvd packet didnt contain frame we were looking for").1.clone();
                            future_pkts.append(&mut updates.clone());
                            future_pkts.retain(|ps| ps.0 > frame_counter);
                            unacked_pkts.retain(|ps| ps.0 > frame_ack);
                            last_rcvd_pkt = frame;
                        },
                        Some(_) => {
                            panic!("Expected UpdateOtherTarget")
                        }
                    }
                }

                let mut m_pressed = false;
                let mut esc_pressed = false;
                loop {
                    // TODO check max_input queue in unsent_pkt.push()
                    if unsent_pkt.len() >= max_input_queue {
                        break;
                    }

                    match rl.get_key_pressed() {
                        Some(k) => {
                            match k {
                                KeyboardKey::KEY_M => { m_pressed = true; }
                                KeyboardKey::KEY_Q => { esc_pressed = true; }
                                KeyboardKey::KEY_SPACE => {
                                    for (u_id, u) in selected_units(&game_state) {
                                        if !u.dead {
                                            if u.cooldown <= 0 {
                                                unsent_pkt.push(GameCommand::Blink(BlinkCommand { u_id }));
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

                mouse_state = match mouse_state {
                    MouseState::None => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            MouseState::Drag(mouse_position)
                        } else if m_pressed { // TODO && gs.selected == Ship
                            MouseState::Path(VecDeque::from(vec![msg_spawn_pos[p_id]]))
                        } else {
                            MouseState::None
                        }
                    },
                    MouseState::Drag(start_pos) => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            MouseState::Drag(start_pos)
                        } else {
                            let selection_pos = Vector2 { x: start_pos.x.min(mouse_position.x), y: start_pos.y.min(mouse_position.y) };
                            let selection_size = Vector2 { x: (start_pos.x - mouse_position.x).abs(), y: (start_pos.y - mouse_position.y).abs() };
                            let uids_in_box: Vec<Selection> = collide_units(&game_state.my_units, &selection_pos, &selection_size).iter().map(|u_id| Selection::Unit(*u_id)).collect();
                            // TODO ship/station selection
                            if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
                                game_state.selection = game_state.selection.symmetric_difference(&HashSet::from_iter(uids_in_box)).cloned().collect();
                            } else {
                                game_state.selection = HashSet::from_iter(uids_in_box);
                            }
                            MouseState::None
                        }
                    },
                    MouseState::Path(mut path) => {
                        if esc_pressed {
                            MouseState::None
                        } else {
                            if rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                                let eff_mouse_pos = mouse_position - MESSAGE_SIZE.scale_by(0.5f32);
                                if let Some(m) = get_manhattan_turn_point(path[path.len() - 1], eff_mouse_pos, p_id) {
                                    path.push_back(m);
                                    path.push_back(eff_mouse_pos);
                                    if station(p_id).collide(&unit_rect(&eff_mouse_pos, MESSAGE_SIZE)) {
                                        unsent_pkt.push(GameCommand::Spawn(SpawnMsgCommand { player_id: p_id, path: path.clone() }));
                                        MouseState::None
                                    } else {
                                        MouseState::Path(path)
                                    }
                                } else {
                                    MouseState::Path(path)
                                }
                            } else {
                                MouseState::Path(path)
                            }
                        }
                    }
                };

                if sent_frame <= frame_counter && (frame_counter % 2 == 0) {
                    unacked_pkts.push_front((frame_counter, unsent_pkt.clone()));
                    socket_send(&socket, &server[0], &ClientPkt::Target { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        updates: unacked_pkts.clone(),
                        frame: frame_counter,
                        frame_ack: last_rcvd_pkt,
                    })?;
                    seq_state.send();
                    frame_state.sent();
                    sent_pkt = unsent_pkt;
                    unsent_pkt = vec![];
                    sent_frame += 2;
                }

                if frame_state == FrameState::Both || (frame_counter % 2 == 1) {
                    if p_id == 0 {
                        apply_updates(&mut game_state.intercepted[p_id], &mut game_state.my_units, &sent_pkt, &mut game_state.other_units, &mut animations);
                        apply_updates(&mut game_state.intercepted[(p_id + 1) % 2], &mut game_state.other_units, &recvd_pkt, &mut game_state.my_units, &mut animations);
                        reap(&mut game_state);
                    } else {
                        apply_updates(&mut game_state.intercepted[(p_id + 1) % 2], &mut game_state.other_units, &recvd_pkt, &mut game_state.my_units, &mut animations);
                        reap(&mut game_state);
                        apply_updates(&mut game_state.intercepted[p_id], &mut game_state.my_units, &sent_pkt, &mut game_state.other_units, &mut animations);
                    }
                    recvd_pkt = vec![];
                    sent_pkt = vec![];
                    game_state.my_units.iter_mut().for_each(|unit| move_unit(unit));
                    game_state.other_units.iter_mut().for_each(|unit| move_unit(unit));
                    add_fuel(&mut game_state, p_id);
                    game_state.fuel.iter_mut().for_each(|f| *f -= FUEL_LOSS);
                    tick_cd_expiry(&mut game_state);
                    frame_counter += 1;
                    if frame_counter % 60 == 0 {
                        socket_send(&socket, &server[0], &ClientPkt::StateHash { 
                            seq: seq_state.send_seq,
                            ack: seq_state.send_ack,
                            hash: crc32fast::hash(&serialize_state(&game_state, p_id).unwrap()),
                            frame: frame_counter,
                        })?;
                        seq_state.send();
                    }
                }

                if frame_state == FrameState::Both {
                    packets_ps.sample();
                    frame_state = FrameState::Neither;
                }

                if game_state.fuel.iter().any(|f| *f <= 0) || game_state.intercepted.iter().any(|v| *v >= KILLS_TO_WIN) {
                    socket_send(&socket, &server[0], &ClientPkt::Ended { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        frame: frame_counter,
                    })?;
                    seq_state.send();
    
                    if game_state.intercepted.iter().all(|v| *v >= KILLS_TO_WIN) || game_state.fuel.iter().all(|f| *f <= 0) {
                        ClientState::Ended(None)
                    } else {
                        if game_state.fuel[0] <= 0 && game_state.fuel[1] > 0 {
                            ClientState::Ended(Some(1usize))
                        } else if game_state.fuel[0] > 0 && game_state.fuel[1] <= 0 {
                            ClientState::Ended(Some(0usize))
                        } else if game_state.intercepted[0] >= KILLS_TO_WIN {
                            ClientState::Ended(Some(0usize))
                        } else {
                            ClientState::Ended(Some(1usize))
                        }
                    }
                } else {
                    ClientState::Started
                }
            },
            ClientState::Ended(end_state) => {
                ended = Some(end_state); // For rendering
                
                if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
                    seq_state = Default::default();
                    ClientState::SendHello
                } else {
                    ClientState::Ended(end_state)
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

        let mut cds = vec![];
        for u in game_state.my_units.iter().chain(game_state.other_units.iter()) {
            let c = if u.player_id == 0 { u.p0_colors() } else { u.p1_colors() };
            d.draw_rectangle_v(u.pos, u.size(), c);
        }

        for s in &game_state.selection {
            match s {
                Selection::Unit(u_id) => {
                    let u = &game_state.my_units[*u_id];
                    d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, u.size().x.round() as i32, u.size().y.round() as i32, Color::BLACK);
                    if !u.path.is_empty() {
                        let mut p = u.pos + u.size().scale_by(0.5f32);
                        let col = rcolor(0, 255, 0, 100);
                        for i in 0..u.path.len() {
                            let next_p = u.path[i] + u.size().scale_by(0.5f32);
                            d.draw_line_v(p, next_p, col);
                            p = next_p;
                        }
                    }
                    cds.push(u.cooldown);
                },
                Selection::Ship => {
                    let rect = ship(p_id);
                    d.draw_rectangle_lines(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2, Color::BLACK)
                },
                Selection::Station => {
                    let rect = station(p_id);
                    d.draw_rectangle_lines(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2, Color::BLACK)
                },
            }
        }   

        if !cds.is_empty() {
            d.draw_text(&format!("CD: {}", cds.iter().min().unwrap()), 20, 60, 20, Color::BLACK);
        }

        for a in animations {
            d.draw_circle(a.x.round() as i32, a.y.round() as i32, INTERCEPT_RADIUS - 10f32, Color::BLACK);
        }
        animations = vec![];

        match mouse_state {
            MouseState::Drag(start_pos) => {
                let selection_pos = Vector2 { x: start_pos.x.min(mouse_position.x), y: start_pos.y.min(mouse_position.y) };
                let selection_size = Vector2 { x: (start_pos.x - mouse_position.x).abs(), y: (start_pos.y - mouse_position.y).abs() };
                d.draw_rectangle_lines(selection_pos.x as i32, selection_pos.y as i32, selection_size.x as i32, selection_size.y as i32, Color::GREEN)
            },
            MouseState::Path(ref path) => {
                let mut p = path[0] + MESSAGE_SIZE.scale_by(0.5f32);
                let col = rcolor(0, 255, 0, 100);
                let bad_col = rcolor(255, 0, 0, 100);
                for i in 1..path.len() {
                    let next_p = path[i] + MESSAGE_SIZE.scale_by(0.5f32);
                    d.draw_line_v(p, next_p, col);
                    p = next_p;
                }

                match get_manhattan_turn_point(p, mouse_position, p_id) {
                    Some(m) => {
                        d.draw_line_v(p, m, col);
                        d.draw_line_v(m, mouse_position, col);
                    }
                    None => {
                        d.draw_line_v(p, mouse_position, bad_col);
                    }
                }
            },
            _ => {}
        }

        d.draw_text(&format!("{:?}", state), 20, 20, 20, Color::BLACK);
        d.draw_text(&fps.to_string(), 20, 40, 20, Color::BLACK);
        d.draw_text(&packets_ps.peek().round().to_string(), 60, 40, 20, Color::BLACK);
        d.draw_text(&format!("{}/{}", game_state.intercepted[p_id], game_state.intercepted[(p_id + 1) % 2]), 20, 100, 20, Color::BLACK);
        if let Some(end_state) = ended {
            let end_str = match end_state {
                Some(winner) => if winner == p_id { "YOU WON" } else { "YOU LOST" },
                None => "DRAW",
            };
            d.draw_text(&end_str, 470, 370, 20, Color::BLACK);
        }
    }
    Ok(())
}