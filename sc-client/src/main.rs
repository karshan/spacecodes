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
mod ui;

use util::*;
use sc_types::constants::*;
use types::*;

use crate::ui::*;

struct Interception {
    start_frame: i64,
    pos: Vector2,
    player_id: usize,
}

fn blink_unit(unit: &mut Unit) -> () {
    unit.blinking.iter_mut().for_each(|b| *b = false);
    if (unit.path[0] - unit.pos).length() < BLINK_RANGE {
        let mut acc = (unit.path[0] - unit.pos).length();
        let mut p0 = unit.path.pop_front().unwrap();
        while !unit.path.is_empty() {
            let p1 = *unit.path.front().unwrap();
            let l = (p1 - p0).length();
            if l + acc >= BLINK_RANGE {
                unit.pos = p0.lerp(p1, (BLINK_RANGE - acc)/l);
                return;
            }
            acc += l;
            p0 = p1;
            unit.path.pop_front();
        }
        unit.pos = p0;
    } else {
        unit.pos += (unit.path[0] - unit.pos).normalized().scale_by(BLINK_RANGE)
    }
}

fn move_unit(unit: &mut Unit) -> () {
    let speed = unit.speed();
    unit.pos =
        if (unit.path[0] - unit.pos).length() < speed {
            // FIXME don't slow down on turns
            unit.path[0]
        } else {
            unit.pos + (unit.path[0] - unit.pos).normalized().scale_by(speed)
        };

    if unit.pos == unit.path[0] {
        unit.path.pop_front();
    }
}

fn move_units(units: &mut Vec<Unit>) {
    units.iter_mut().for_each(|unit|
        match unit.blinking {
            Some(true) => blink_unit(unit),
            _ => move_unit(unit)
        }
    );
}

fn apply_updates(game_state: &mut GameState, updates: [&Vec<GameCommand>; 2], p_id: usize, interceptions: &mut Vec<Interception>, frame: i64) {
    for i in 0..=1 {
        for u in updates[i] {
            let units = if p_id == i { &mut game_state.my_units } else { &mut game_state.other_units };
            match u {
                GameCommand::Blink(BlinkCommand { u_id }) => {
                    if *u_id < units.len() {
                        units[*u_id].cooldown = units[*u_id].cooldown();
                        units[*u_id].blinking = Some(true);
                    }
                },
                GameCommand::Spawn(SpawnMsgCommand { path, player_id, blink_imbued }) => {
                    if *blink_imbued {
                        game_state.items[i].entry(Item::Blink).and_modify(|e| *e -= 1).or_insert(-1);
                    }
                    units.push(Unit { dead: false, player_id: *player_id, pos: path[0], path: path.clone(), blinking: if *blink_imbued { Some(false) } else { None }, cooldown: 0 });
                },
                GameCommand::Intercept(InterceptCommand { pos }) => {
                    interceptions.push(Interception { pos: pos.clone(), start_frame: frame, player_id: i });
                    game_state.gold[i] -= INTERCEPT_COST;
                },
                GameCommand::BuyUpgrade(u) => {
                    game_state.upgrades[i].insert(*u);
                    game_state.gold[i] -= u.cost();
                },
                GameCommand::BuyItem(item) => {
                    game_state.items[i].entry(*item).and_modify(|e| *e += 1).or_insert(1);
                    game_state.gold[i] -= item.cost();
                }
            }
        }
    }

    for intercept in &mut *interceptions {
        if (frame - intercept.start_frame) as f32 >= INTERCEPT_DELAY {
            let other_units = if p_id == intercept.player_id { &mut game_state.other_units } else { &mut game_state.my_units };
            for unit in other_units.iter_mut() {
                // Have to check unit.dead to avoid double counting interception kills (If 2 interceptions kill the same unit on the same frame)
                if !unit.dead {
                    let unit_cen = unit.pos + unit.size().scale_by(0.5f32);
                    if (unit_cen.x - intercept.pos.x).powf(2f32) + (unit_cen.y - intercept.pos.y).powf(2f32) <= INTERCEPT_RADIUS.powf(2f32) {
                        unit.dead = true;
                        game_state.intercepted[intercept.player_id] += 1;
                    }
                }
            }
        }
    }
    interceptions.retain(|i| ((frame - i.start_frame) as f32) < INTERCEPT_DELAY);
    reap(game_state);
    game_state.other_units.retain(|u| !u.dead);
}

fn add_fuel(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    game_state.my_units.iter_mut().filter(|u| u.rect().collide(station(u.player_id)))
        .for_each(|u| u.dead = true);
    reap(game_state);
    game_state.other_units.retain(|u| !u.rect().collide(station(u.player_id)));

    game_state.fuel[p_id] = min(START_FUEL, game_state.fuel[p_id] + (num_my_units - game_state.my_units.len() as i32) * MSG_FUEL);
    game_state.fuel[other_id] = min(START_FUEL, game_state.fuel[other_id] + (num_other_units - game_state.other_units.len() as i32) * MSG_FUEL);
}

fn tick(game_state: &mut GameState) {
    for u in game_state.my_units.iter_mut().chain(game_state.other_units.iter_mut()) {
        u.cooldown = max(0, u.cooldown - 1);
    }

    game_state.fuel.iter_mut().for_each(|f| *f -= FUEL_LOSS);
    game_state.gold.iter_mut().for_each(|g| *g += GOLD_GAIN);
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
    let mut choices = vec![];
    if game_state.selection.iter().any(|s| if let Selection::Unit(_) = s { true } else { false }) {
        choices.push(SubSelection::Unit);
    }
    if game_state.selection.contains(&Selection::Ship) {
        choices.push(SubSelection::Ship);
    }
    if game_state.selection.contains(&Selection::Station) {
        choices.push(SubSelection::Station);
    }
    if let Some(cur_subsel) = game_state.sub_selection {
        if !choices.contains(&cur_subsel) {
            game_state.sub_selection = if choices.is_empty() { None } else { Some(choices[0]) };
        }
    }
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
    let p1_ok = PLAY_AREA.contains(&unit_rect(&p1, MESSAGE_SIZE));
    let p2_ok = PLAY_AREA.contains(&unit_rect(&p2, MESSAGE_SIZE));
    if !p1_ok || !p2_ok {
        None
    } else {
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
    let intercept_colors = [rcolor(0x90, 0xE0, 0xEF, 100), rcolor(0x74, 0xC6, 0x9D, 100)];
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
        .size(1024, 968)
        .title("Space Codes")
        .build();
    rl.set_target_fps(frame_rate);
    let message_spell_icons = MessageSpellIcons::new(&mut rl, &thread);
    let ship_spell_icons = ShipSpellIcons::new(&mut rl, &thread);

    let mut state = ClientState::SendHello;
    // Most of these values doesn't matter. Its just for the compiler. They are initialized in ClientState::Waiting
    let mut game_state: GameState = GameState {
        my_units: vec![],
        other_units: vec![],
        selection: HashSet::new(),
        sub_selection: None,
        fuel: [START_FUEL; 2],
        intercepted: [0; 2],
        gold: [0f32; 2],
        upgrades: [HashSet::new(), HashSet::new()],
        items: [HashMap::new(), HashMap::new()]
    };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    // TODO All this netcode related stuff should be abstracted into a single type
    let mut sent_frame = 0;
    let mut frame_state = FrameState::Neither;
    let mut unsent_pkt = vec![];
    let mut unacked_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut future_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut sent_pkt = vec![];
    let mut recvd_pkt = vec![];
    let mut last_rcvd_pkt = -1;
    // ------------------------
    let mut interceptions = vec![];
    enum MouseState {
        Drag(Vector2),
        Path(VecDeque<Vector2>, bool),
        Intercept,
        WaitReleaseLButton,
        None
    }
    let mut mouse_state: MouseState = MouseState::None;
    let mut ended = None;
    let mut packets_ps = WindowAvg::new();
    let mut shop_open = false;
    let shop = Shop::new(&mut rl, &thread).unwrap();

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
                        interceptions = vec![];
                        mouse_state = MouseState::None;
                        frame_state = FrameState::Neither;
                        shop_open = false;
                        game_state = GameState {
                            my_units: vec![],
                            other_units: vec![],
                            selection: HashSet::new(),
                            sub_selection: None,
                            fuel: [START_FUEL; 2],
                            intercepted: [0; 2],
                            gold: [0f32; 2],
                            upgrades: [HashSet::new(), HashSet::new()],
                            items: [HashMap::new(), HashMap::new()]
                        };
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

                let mut start_message_path: Option<bool> = None;
                let mut cancel = false;
                let mut start_intercept = false;
                loop {
                    // TODO check max_input queue in unsent_pkt.push()
                    if unsent_pkt.len() >= max_input_queue {
                        break;
                    }

                    match rl.get_key_pressed() {
                        Some(k) => {
                            match k {
                                KeyboardKey::KEY_TAB => {
                                    if let Some(subsel) = game_state.sub_selection {
                                        let mut choices = vec![];
                                        if game_state.selection.iter().any(|s| if let Selection::Unit(_) = s { true } else { false }) {
                                            choices.push(SubSelection::Unit);
                                        }
                                        if game_state.selection.contains(&Selection::Ship) {
                                            choices.push(SubSelection::Ship);
                                        }
                                        if game_state.selection.contains(&Selection::Station) {
                                            choices.push(SubSelection::Station);
                                        }
                                        game_state.sub_selection = Some(choices[(choices.iter().enumerate().find(|(_, c)| **c == subsel).unwrap().0 + 1) % choices.len()]);
                                    }
                                },
                                KeyboardKey::KEY_M => {
                                    if game_state.sub_selection == Some(SubSelection::Ship) {
                                        start_message_path = Some(false)
                                    }
                                },
                                KeyboardKey::KEY_B => {
                                    if game_state.sub_selection == Some(SubSelection::Ship) &&
                                        *game_state.items[p_id].entry(Item::Blink).or_insert(0) > 0 {
                                        start_message_path = Some(true)
                                    }
                                },
                                KeyboardKey::KEY_I => { start_intercept = game_state.sub_selection == Some(SubSelection::Ship) },
                                KeyboardKey::KEY_S => { shop_open = !shop_open }
                                KeyboardKey::KEY_Q => {
                                    match mouse_state {
                                        MouseState::Path(_, _) => { cancel = true }
                                        MouseState::Intercept => { cancel = true }
                                        _ => {}
                                    }
                                },
                                KeyboardKey::KEY_SPACE => {
                                    for (u_id, u) in selected_units(&game_state) {
                                        if u.cooldown <= 0 && u.blinking.is_some() {
                                            unsent_pkt.push(GameCommand::Blink(BlinkCommand { u_id }));
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                        None => break
                    }
                }

                // TODO && contains_point(SHOP_AREA, mouse_position)
                if shop_open && rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                    if let Some(shop_item) = shop.click(mouse_position) {
                        match shop_item {
                            ShopItem::Item(i) => {
                                if game_state.gold[p_id] >= i.cost() {
                                    unsent_pkt.push(GameCommand::BuyItem(i))
                                }
                            },
                            ShopItem::Upgrade(u) => {
                                if !game_state.upgrades[p_id].contains(&u) &&
                                        game_state.gold[p_id] >= u.cost() {
                                    unsent_pkt.push(GameCommand::BuyUpgrade(u))
                                }
                            }
                        }
                    }
                }

                mouse_state = match mouse_state {
                    MouseState::None => {
                        if contains_point(&PLAY_AREA, &mouse_position) && rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            MouseState::Drag(mouse_position)
                        } else if start_message_path.is_some() {
                            MouseState::Path(VecDeque::from(vec![msg_spawn_pos[p_id]]), start_message_path.unwrap())
                        } else if start_intercept {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_CROSSHAIR);
                            MouseState::Intercept
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
                            let selection_rect = Rect { x: selection_pos.x.round() as i32, y: selection_pos.y.round() as i32, w: selection_size.x.round() as i32, h: selection_size.y.round() as i32 };
                            let mut in_box: Vec<Selection> = collide_units(&game_state.my_units, &selection_pos, &selection_size).iter().map(|u_id| Selection::Unit(*u_id)).collect();
                            if ship(p_id).collide(&selection_rect) {
                                in_box.push(Selection::Ship);
                            }
                            if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
                                game_state.selection = game_state.selection.symmetric_difference(&HashSet::from_iter(in_box)).cloned().collect();
                            } else {
                                game_state.selection = HashSet::from_iter(in_box);
                            }
                            let mut choices = vec![];
                            if game_state.selection.iter().any(|s| if let Selection::Unit(_) = s { true } else { false }) {
                                choices.push(SubSelection::Unit);
                            }
                            if game_state.selection.contains(&Selection::Ship) {
                                choices.push(SubSelection::Ship);
                            }
                            if game_state.selection.contains(&Selection::Station) {
                                choices.push(SubSelection::Station);
                            }
                            if choices.is_empty() {
                                game_state.sub_selection = None;
                            } else {
                                game_state.sub_selection = Some(choices[0]);
                            }
                            MouseState::None
                        }
                    },
                    MouseState::Path(mut path, blink_imbued) => {
                        if cancel {
                            MouseState::None
                        } else {
                            if contains_point(&PLAY_AREA, &mouse_position) && rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                                let eff_mouse_pos = mouse_position - MESSAGE_SIZE.scale_by(0.5f32);
                                if let Some(m) = get_manhattan_turn_point(path[path.len() - 1], eff_mouse_pos, p_id) {
                                    path.push_back(m);
                                    path.push_back(eff_mouse_pos);
                                    if station(p_id).collide(&unit_rect(&eff_mouse_pos, MESSAGE_SIZE)) {
                                        unsent_pkt.push(GameCommand::Spawn(SpawnMsgCommand { player_id: p_id, path: path.clone(), blink_imbued: blink_imbued }));
                                        MouseState::WaitReleaseLButton
                                    } else {
                                        MouseState::Path(path, blink_imbued)
                                    }
                                } else {
                                    MouseState::Path(path, blink_imbued)
                                }
                            } else {
                                MouseState::Path(path, blink_imbued)
                            }
                        }
                    },
                    MouseState::Intercept => {
                        if cancel {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                            MouseState::None
                        } else if contains_point(&PLAY_AREA, &mouse_position) && rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            if game_state.gold[p_id] >= INTERCEPT_COST {
                                unsent_pkt.push(GameCommand::Intercept(InterceptCommand { pos: mouse_position }));
                                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                                MouseState::WaitReleaseLButton
                            } else {
                                MouseState::Intercept
                            }
                        } else {
                            MouseState::Intercept
                        }
                    },
                    MouseState::WaitReleaseLButton => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            MouseState::WaitReleaseLButton
                        } else {
                            MouseState::None
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
                    apply_updates(&mut game_state, if p_id == 0 { [&sent_pkt, &recvd_pkt] } else { [&recvd_pkt, &sent_pkt] }, p_id, &mut interceptions, frame_counter);
                    recvd_pkt = vec![];
                    sent_pkt = vec![];
                    move_units(&mut game_state.my_units);
                    move_units(&mut game_state.other_units);
                    add_fuel(&mut game_state, p_id);
                    tick(&mut game_state);
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

        for u in game_state.my_units.iter().chain(game_state.other_units.iter()) {
            let c = if u.player_id == 0 { u.p0_colors() } else { u.p1_colors() };
            if u.blinking.is_some() {
                d.draw_circle_v(u.pos + u.size().scale_by(0.5f32), u.size().x/2f32, c);
            } else {
                d.draw_rectangle_v(u.pos, u.size(), c);
            }
        }

        for s in &game_state.selection {
            match s {
                Selection::Unit(u_id) => {
                    let u = &game_state.my_units[*u_id];
                    if u.blinking.is_some() {
                        let cen = u.pos + u.size().scale_by(0.5f32);
                        d.draw_circle_lines(cen.x.round() as i32, cen.y.round() as i32, u.size().x/2f32, Color::BLACK);
                    } else {
                        d.draw_rectangle_lines(u.pos.x.round() as i32, u.pos.y.round() as i32, u.size().x.round() as i32, u.size().y.round() as i32, Color::BLACK);
                    }
                    if !u.path.is_empty() {
                        let mut p = u.pos + u.size().scale_by(0.5f32);
                        let col = rcolor(0, 255, 0, 100);
                        for i in 0..u.path.len() {
                            let next_p = u.path[i] + u.size().scale_by(0.5f32);
                            d.draw_line_v(p, next_p, col);
                            p = next_p;
                        }
                    }
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

        if game_state.sub_selection == Some(SubSelection::Unit) {
            let cooldowns: Vec<i32> =
                game_state.selection.iter().map(
                    |s| if let Selection::Unit(uid) = s {
                        Some(game_state.my_units[*uid].cooldown)
                    } else { None }
                ).flatten().collect();

            message_spell_icons.render(&mut d, (*cooldowns.iter().min().unwrap() as f32)/(BLINK_COOLDOWN as f32));
        } else if game_state.sub_selection == Some(SubSelection::Ship) {
            ship_spell_icons.render(&mut d, 0f32);
        }

        for a in &interceptions {
            // intercept radius - 10f32 is a hack to make it look like and intercept just clipping a message is successful
            // actually the interception circle needs to contain the center of the message to succeed
            d.draw_circle_v(a.pos, ((INTERCEPT_RADIUS - 10f32) * ((frame_counter - a.start_frame) as f32))/INTERCEPT_DELAY, intercept_colors[a.player_id]);
            d.draw_circle_lines(a.pos.x as i32, a.pos.y as i32, INTERCEPT_RADIUS - 10f32, Color::BLACK);
        }

        match mouse_state {
            MouseState::Drag(start_pos) => {
                let selection_pos = Vector2 { x: start_pos.x.min(mouse_position.x), y: start_pos.y.min(mouse_position.y) };
                let selection_size = Vector2 { x: (start_pos.x - mouse_position.x).abs(), y: (start_pos.y - mouse_position.y).abs() };
                d.draw_rectangle_lines(selection_pos.x as i32, selection_pos.y as i32, selection_size.x as i32, selection_size.y as i32, Color::GREEN)
            },
            MouseState::Path(ref path, blink_imbued) => {
                let mut p = path[0] + MESSAGE_SIZE.scale_by(0.5f32);
                let col = if blink_imbued { rcolor(0, 0, 255, 100) } else { rcolor(0, 255, 0, 100) };
                let bad_col = rcolor(255, 0, 0, 100);
                for i in 1..path.len() {
                    let next_p = path[i] + MESSAGE_SIZE.scale_by(0.5f32);
                    d.draw_line_v(p, next_p, col);
                    p = next_p;
                }

                let eff_mouse_pos = mouse_position - MESSAGE_SIZE.scale_by(0.5f32);
                match get_manhattan_turn_point(p - MESSAGE_SIZE.scale_by(0.5f32), eff_mouse_pos, p_id) {
                    Some(m) => {
                        d.draw_line_v(p, m + MESSAGE_SIZE.scale_by(0.5f32), col);
                        d.draw_line_v(m + MESSAGE_SIZE.scale_by(0.5f32), mouse_position, col);
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

        d.draw_line(0, 768, 1023, 768, Color::BLACK);
        d.draw_text(&format!("{:?}", game_state.sub_selection), 20, 768, 20, Color::BLACK);
        d.draw_text(&format!("{}", game_state.gold[p_id].round()), 20, 788, 20, Color::BLACK);
        if shop_open {
            shop.render(&mut d, &game_state.upgrades[p_id], game_state.gold[p_id]);
        }
        d.draw_text(&format!("{:?}", game_state.upgrades[p_id]), 20, 808, 20, Color::BLACK);
        d.draw_text(&format!("{:?}", game_state.items[p_id]), 20, 828, 20, Color::BLACK);
    }
    Ok(())
}