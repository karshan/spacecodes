use std::collections::{HashMap, HashSet};
use std::cmp::{min, max};
use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use petgraph::algo::astar;
use raylib::prelude::*;
use sc_types::*;
use sc_types::shapes::*;
extern crate rmp_serde as rmps;
use petgraph::{Graph, Undirected};

mod util;
mod constants;

use util::*;
use constants::*;

#[derive(Debug)]
enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started,
    Ended(Option<usize>),
}

#[derive(Eq, PartialEq)]
enum FrameState {
    Neither,
    Sent,
    Received,
    Both,
}

impl FrameState {
    fn recvd(self: &mut Self) {
        match self {
            FrameState::Neither => *self = FrameState::Received,
            FrameState::Sent => *self = FrameState::Both,
            _ => {},
        }
    }

    fn sent(self: &mut Self) {
        match self {
            FrameState::Neither => *self = FrameState::Sent,
            FrameState::Received => *self = FrameState::Both,
            _ => {},
        }
    }
}

fn path_collides(rects: &[Rect<i32>], offsets: [Vector2; 4], pos: Vector2, target: Vector2) -> bool {
    let mut collided = false;
    for r in rects {
        for l in r.lines() {
            for o in offsets {
                if let Some(_) = raylib::check_collision_lines(pos + o, target + o, l[0], l[1]) {
                    collided = true;
                    break;
                }
            }
            if collided { break; }
        }
        if collided { break; }
    }
    collided
}

fn base_graph(unit_type: UnitEnum) -> Graph<Vector2, f32, Undirected> {
    let mut g = Graph::<Vector2, f32, Undirected>::new_undirected();
    let blocked = &GAME_MAP[0].1;
    let sp0 = &GAME_MAP[1].1;
    let st0 = &GAME_MAP[2].1;
    let sp1 = &GAME_MAP[3].1;
    let st1 = &GAME_MAP[4].1;
    /*
     *   n0--sp00--sp01--n3
     *    |              |
     *   st10           sp10
     *    |              |
     *   st11           sp11
     *    |              |
     *   n1--st00--st01--n2
     */
    let n0 = g.add_node(Vector2 { x: blocked.x as f32, y: blocked.y as f32 } - *unit_type.size() - Vector2::one());
    let n1 = g.add_node(Vector2 { x: blocked.x as f32, y: (blocked.y + blocked.h) as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let n2 = g.add_node(Vector2 { x: (blocked.x + blocked.w) as f32, y: (blocked.y + blocked.h) as f32 } + Vector2::one());
    let n3 = g.add_node(Vector2 { x: (blocked.x + blocked.w) as f32, y: blocked.y as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let sp00 = g.add_node(Vector2 { x: sp0.x as f32, y: sp0.y as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let sp01 = g.add_node(Vector2 { x: (sp0.x + sp0.w) as f32, y: sp0.y as f32 } - *unit_type.size() - Vector2::one());
    let st00 = g.add_node(Vector2 { x: st0.x as f32, y: (st0.y + st0.h) as f32 } + Vector2::one());
    let st01 = g.add_node(Vector2 { x: (st0.x + st0.w) as f32, y: (st0.y + st0.h) as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let sp10 = g.add_node(Vector2 { x: (sp1.x + sp1.w) as f32, y: sp1.y as f32 } + Vector2::one());
    let sp11 = g.add_node(Vector2 { x: (sp1.x + sp1.w) as f32, y: (sp1.y + sp1.h) as f32 } + Vector2 { x: 1f32, y: -unit_type.size().y - 1f32 });
    let st10 = g.add_node(Vector2 { x: st1.x as f32, y: st1.y as f32 } + Vector2 { x: -unit_type.size().x - 1f32, y: 1f32 });
    let st11 = g.add_node(Vector2 { x: st1.x as f32, y: (st1.y + st1.h) as f32 } - *unit_type.size() - Vector2::one());
    g.add_edge(n0, sp00, (g[n0] - g[sp00]).length());
    g.add_edge(sp00, sp01, (g[sp00] - g[sp01]).length());
    g.add_edge(sp01, n3, (g[sp01] - g[n3]).length());
    g.add_edge(n1, st00, (g[n1] - g[st00]).length());
    g.add_edge(st00, st01, (g[st00] - g[st01]).length());
    g.add_edge(st01, n2, (g[st01] - g[n2]).length());
    g.add_edge(n2, sp11, (g[n2] - g[sp11]).length());
    g.add_edge(n3, sp10, (g[n3] - g[sp10]).length());
    g.add_edge(sp10, sp11, (g[sp10] - g[sp11]).length());
    g.add_edge(n0, st10, (g[n0] - g[st10]).length());
    g.add_edge(st10, st11, (g[st10] - g[st11]).length());
    g.add_edge(st11, n1, (g[st11] - g[n1]).length());
    g
}

fn find_paths(units: &mut Vec<Unit>) {
    for u in units {
        if u.path.len() > 0 {
            continue;
        }

        let offsets = [ Vector2::zero(), *u.type_.size(), Vector2 { x: u.type_.size().x, y: 0f32 }, Vector2 { x: 0f32, y: u.type_.size().y } ];
        let rects = if u.player_id == 0 { &P0_BLOCKED } else { &P1_BLOCKED };
        
        if !path_collides(rects, offsets, u.pos, u.target) {
            u.path.push((u.target.x, u.target.y));
        } else {
            let mut g = base_graph(u.type_);
            let start_node = g.add_node(u.pos);
            for n in g.node_indices() {
                if n == start_node { continue; }
                if !path_collides(rects, offsets, u.pos, g[n]) {
                    g.add_edge(n, start_node, (g[n] - u.pos).length());
                }
            }
            let end_node = g.add_node(u.target);
            for n in g.node_indices() {
                if n == start_node || n == end_node { continue; }
                if !path_collides(rects, offsets, u.target, g[n]) {
                    g.add_edge(n, end_node, (g[n] - u.target).length());
                }
            }
            let path = astar(&g, start_node, |n| n == end_node, |e| *e.weight(), |n| (g[n] - u.target).length());
            match path {
                None => {
                    u.path = vec![(u.target.x, u.target.y)]
                },
                Some(mut p) => {
                    p.1.reverse();
                    u.path = p.1.iter().map(|n| (g[*n].x, g[*n].y)).collect();
                }
            }       
        }
    }
}

fn move_unit(unit: Unit, speed: f32) -> Vector2 {
    if unit.path.is_empty() {
        return unit.pos;
    }
    let target = Vector2 { x: unit.path[unit.path.len() - 1].0, y: unit.path[unit.path.len() - 1].1 };
    let new_pos = if (target - unit.pos).length_sqr() < speed * speed {
            target
        } else {
            unit.pos + (target - unit.pos).normalized().scale_by(speed)
        };
    let new_unit_rect = (Unit { pos: new_pos, ..unit.clone() }).rect();
    let play_area = Rect { x: 0, y: 0, w: 1024, h: 768 };
    let blocked_rects = if unit.player_id == 0 { &P0_BLOCKED } else { &P1_BLOCKED };
    for r in blocked_rects {
        if new_unit_rect.collide(r) {
            return unit.pos;
        }
    }
    if !play_area.contains(&new_unit_rect) {
        unit.pos
    } else {
        new_pos
    }
}

fn apply_updates(intercepted_count: &mut u8, units: &mut Vec<Unit>, updates: &[GameCommand], other_units: &mut Vec<Unit>, animations: &mut Vec<Vector2>) {
    for u in updates {
        match u {
            GameCommand::Move(MoveCommand { u_id, target }) => {
                units[*u_id].target = *target;
                units[*u_id].path = vec![];
            },
            GameCommand::Spawn(SpawnCommand { unit_type, spawn_pos, player_id }) => {
                units.push(Unit { type_: *unit_type, player_id: *player_id, pos: *spawn_pos, target: *spawn_pos, path: vec![], cooldown: 0 });
            },
            GameCommand::Intercept(InterceptCommand { u_id, pos }) => {
                units[*u_id].cooldown = UnitEnum::Interceptor(0).cooldown();
                animations.push(pos.clone());
                for unit in other_units.iter_mut() {
                    match unit.type_ {
                        UnitEnum::MessageBox => {
                            let unit_cen = unit.pos + unit.type_.size().scale_by(0.5f32);
                            if (unit_cen.x - pos.x).powf(2f32) + (unit_cen.y - pos.y).powf(2f32) <= INTERCEPT_RADIUS.powf(2f32) {
                                unit.type_ = UnitEnum::Dead;
                                *intercepted_count += 1;
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

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    // FIXME need to be able to lookup ship/station rects
    game_state.my_units.iter_mut().filter(|u| if let UnitEnum::MessageBox = u.type_ { u.rect().collide(&GAME_MAP[2 + u.player_id * 2].1) } else { false })
        .for_each(|u| u.type_ = UnitEnum::Dead);
    reap(game_state);
    game_state.other_units.retain(|u| if let UnitEnum::MessageBox = u.type_ { !u.rect().collide(&GAME_MAP[2 + u.player_id * 2].1) } else { true });

    game_state.fuel[p_id] = min(START_FUEL, game_state.fuel[p_id] + (num_my_units - game_state.my_units.len() as i32) * MSG_FUEL);
    game_state.fuel[other_id] = min(START_FUEL, game_state.fuel[other_id] + (num_other_units - game_state.other_units.len() as i32) * MSG_FUEL);
}

fn tick_cd_expiry(game_state: &mut GameState) {
    for u in game_state.my_units.iter_mut().chain(game_state.other_units.iter_mut()) {
        u.cooldown = max(0, u.cooldown - 1);
        if let UnitEnum::Interceptor(e) = u.type_ {
            if e == 1 {
                u.type_ = UnitEnum::Dead
            } else {
                u.type_ = UnitEnum::Interceptor(max(0, e - 1))
            }
        }
    }

    reap(game_state);
    game_state.other_units.retain(|u| u.type_ != UnitEnum::Dead);
}

fn collide_units(units: &Vec<Unit>, p: &Vector2, s: &Vector2) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];
    for (i, u) in units.iter().enumerate() {
        if (Rect { x: p.x, y: p.y, w: s.x, h: s.y }).collide(&Rect { x: u.pos.x, y: u.pos.y, w: u.type_.size().x, h: u.type_.size().y }) {
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
        let spawn_command = Some(GameCommand::Spawn(SpawnCommand { unit_type: t, spawn_pos: spawn_pos[player_id as usize], player_id: player_id }));
        if let UnitEnum::Interceptor(_) = t {
            if game_state.my_units.iter().filter(|u| if let UnitEnum::Interceptor(_) = u.type_ { true } else { false }).count() >= MAX_INTERCEPTORS {
                None
            } else {
                spawn_command
            }
        } else {
            spawn_command
        }
    }
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

fn move_units(units: &mut Vec<Unit>) {
    let moved: Vec<_> = units.iter().cloned().map(|unit| Unit { pos: move_unit(unit.clone(), unit.type_.speed()), ..unit }).collect();
    for i in 0..moved.len() {
        let mut collided = vec![];
        for j in 0..moved.len() {
            if i == j {
                continue;
            }
            if moved[i].rect().collide(&moved[j].rect()) {
                collided.push(moved[j].clone());
            }
        }
        if collided.is_empty() {
            units[i] = moved[i].clone();
            if !units[i].path.is_empty() {
                let path_target = units[i].path[units[i].path.len() - 1];
                if (Vector2 { x: path_target.0, y: path_target.1 }) == units[i].pos {
                    units[i].path.pop();
                }
            }
        } else {
            collided.push(moved[i].clone());
            let num_collided = collided.len();
            let mut sum = Vector2::zero();
            for c in collided {
                sum += c.pos;
            }
            let center = sum.scale_by(1f32/(num_collided as f32));
            let pushed_pos = units[i].pos + (units[i].pos - center).normalized().scale_by(units[i].type_.speed());
            units[i].path.push((pushed_pos.x, pushed_pos.y));
        }
    }
}

fn reap(game_state: &mut GameState) {
    let mut out = HashSet::new();
    for s in &game_state.selection {
        if let Selection::Unit(selection_uid) = s {
            match game_state.my_units[*selection_uid].type_ {
                UnitEnum::Dead => {}
                _ => {
                    let mut count_dead = 0;
                    for i in 0..*selection_uid {
                        if let UnitEnum::Dead = game_state.my_units[i].type_ {
                            count_dead += 1;
                        }
                    }
                    out.insert(Selection::Unit(*selection_uid - count_dead));
                }
            }
        } else {
            out.insert(*s);
        }
    }
    game_state.selection = out;
    game_state.my_units.retain(|u| u.type_ != UnitEnum::Dead);
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
    let int_spawn_pos = [Vector2 { x: 502f32, y: 499f32 }, Vector2 { x: 377f32, y: 374f32 }];


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
    // Most of these values doesn't matter. Its just for the compiler. They are initialized in ClientState::Waiting
    let mut game_state: GameState = GameState { my_units: vec![], other_units: vec![], selection: HashSet::new(), fuel: [START_FUEL; 2], intercepted: [0; 2] };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    let mut s_time = 0f64;
    let mut sent_frame = 0;
    let mut frame_state = FrameState::Neither;
    let mut unsent_pkt = vec![];
    let mut sent_pkt = vec![];
    let mut recvd_pkt = vec![];
    let mut animations = vec![];
    let mut drag_select: Option<Vector2> = None;
    let mut ended = None;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mouse_position = rl.get_mouse_position();
        let fps = rl.get_fps();

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
                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => ClientState::Waiting,
                    Some(ServerEnum::Start) => {
                        frame_counter = 0;
                        sent_frame = 0;
                        unsent_pkt = vec![];
                        ended = None;
                        animations = vec![];
                        drag_select = None;
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
                if frame_counter % 2 == 0 {
                    let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                    match resp {
                        None => {},
                        Some(ServerEnum::UpdateOtherTarget { updates, frame: _ }) => {
                            frame_state.recvd();
                            recvd_pkt = updates;
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
                            let uids_in_box: Vec<Selection> = collide_units(&game_state.my_units, &selection_pos, &selection_size).iter().map(|u_id| Selection::Unit(*u_id)).collect();
                            if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
                                game_state.selection = game_state.selection.symmetric_difference(&HashSet::from_iter(uids_in_box)).cloned().collect();
                            } else {
                                game_state.selection = HashSet::from_iter(uids_in_box);
                            }
                            None
                        }
                    }
                };

                if rl.is_mouse_button_pressed(MouseButton::MOUSE_RIGHT_BUTTON) && unsent_pkt.len() < max_input_queue {
                    for (u_id, u) in selected_units(&game_state) {
                        unsent_pkt.push(GameCommand::Move(MoveCommand { u_id: u_id, target: rl.get_mouse_position() - u.type_.size().scale_by(0.5f32) }));
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
                                    for (u_id, u) in selected_units(&game_state) { 
                                        unsent_pkt.push(GameCommand::Move(MoveCommand { u_id: u_id, target: u.pos })); 
                                    }
                                },
                                KeyboardKey::KEY_M => { spawn(&game_state, p_id, UnitEnum::MessageBox, &msg_spawn_pos).map(|c| unsent_pkt.push(c)); }
                                KeyboardKey::KEY_I => { spawn(&game_state, p_id, UnitEnum::Interceptor(INTERCEPTOR_EXPIRY), &msg_spawn_pos).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_O => { spawn(&game_state, p_id, UnitEnum::Interceptor(INTERCEPTOR_EXPIRY), &int_spawn_pos).map(|c| unsent_pkt.push(c)); },
                                KeyboardKey::KEY_ONE => game_state.selection = HashSet::from([Selection::Ship]),
                                KeyboardKey::KEY_TWO => game_state.selection = HashSet::from([Selection::Station]),
                                KeyboardKey::KEY_SPACE => {
                                    for (u_id, u) in selected_units(&game_state) {
                                        if let UnitEnum::Interceptor(_) = u.type_ {
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
                    if p_id == 0 {
                        find_paths(&mut game_state.my_units);
                        find_paths(&mut game_state.other_units);
                        move_units(&mut game_state.my_units);
                        move_units(&mut game_state.other_units);
                    } else {
                        find_paths(&mut game_state.other_units);
                        find_paths(&mut game_state.my_units);
                        move_units(&mut game_state.other_units);
                        move_units(&mut game_state.my_units);
                    }
                    add_fuel(&mut game_state, p_id);
                    game_state.fuel.iter_mut().for_each(|f| *f -= FUEL_LOSS);
                    tick_cd_expiry(&mut game_state);
                    frame_counter += 1;
                    frame_state = FrameState::Neither;
                    socket_send(&socket, &server[0], &ClientPkt::StateHash { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        hash: crc32fast::hash(&serialize_state(&game_state, p_id).unwrap()),
                        frame: frame_counter,
                    })?;
                    seq_state.send();
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
        let mut exps = vec![];
        for u in game_state.my_units.iter().chain(game_state.other_units.iter()) {
            let c = if u.player_id == 0 { u.type_.p0_colors() } else { u.type_.p1_colors() };
            match u.type_ {
                UnitEnum::Interceptor(_) => {
                    let cen = u.pos + u.type_.size().scale_by(0.5f32);
                    d.draw_circle(cen.x.round() as i32, cen.y.round() as i32, u.type_.size().x/2f32, c);
                },
                UnitEnum::MessageBox => d.draw_rectangle_v(u.pos, u.type_.size(), c),
                _ => {}
            }
        }

        for s in &game_state.selection {
            match s {
                Selection::Unit(u_id) => {
                    let u = &game_state.my_units[*u_id];
                    match u.type_ {
                        UnitEnum::Interceptor(e) => {
                            let cen = u.pos + u.type_.size().scale_by(0.5f32);
                            d.draw_circle_lines(cen.x.round() as i32, cen.y.round() as i32, u.type_.size().x/2f32, Color::BLACK);
                            exps.push(e);
                        },
                        UnitEnum::MessageBox => {
                            d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, u.type_.size().x.round() as i32, u.type_.size().y.round() as i32, Color::BLACK)
                        },
                        _ => {}
                    }
                    if !u.path.is_empty() {
                        let mut p = Vector2 { x: u.path[0].0, y: u.path[0].1 } + u.type_.size().scale_by(0.5f32);
                        let col = rcolor(0, 255, 0, 100);
                        for i in 1..u.path.len() {
                            let next_p = Vector2 { x: u.path[i].0, y: u.path[i].1 } + u.type_.size().scale_by(0.5f32);
                            d.draw_line_v(p, next_p, col);
                            p = next_p;
                        }
                        let last_p = u.pos + u.type_.size().scale_by(0.5f32);
                        d.draw_line_v(p, last_p, col);
                    }
                    cds.push(u.cooldown);
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
            }
        }   

        if !cds.is_empty() {
            d.draw_text(&format!("CD: {}", cds.iter().min().unwrap()), 20, 60, 20, Color::BLACK);
        }
        if !exps.is_empty() {
            d.draw_text(&format!("E: {}", exps.iter().min().unwrap()), 20, 80, 20, Color::BLACK);
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

        d.draw_text(&format!("{:?}", state), 20, 20, 20, Color::BLACK);
        d.draw_text(&fps.to_string(), 20, 40, 20, Color::BLACK);
        d.draw_text(&format!("{}/{}", game_state.intercepted[p_id], game_state.intercepted[(p_id + 1) % 2]), 20, 100, 20, Color::BLACK);
        if let Some(end_state) = ended {
            let end_str = match end_state {
                Some(winner) => {
                    if winner == p_id {
                        "YOU WON"
                    } else {
                        "YOU LOST"
                    }
                },
                None => {
                    "DRAW"
                }
            };
            d.draw_text(&end_str, 470, 370, 20, Color::BLACK);
        }
    }
    Ok(())
}