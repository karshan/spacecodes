use std::collections::{HashMap, HashSet, VecDeque};
use std::cmp::{min, max};
use std::net::UdpSocket;
use crate::net::NetState;
use raylib::prelude::*;
use sc_types::*;
use sc_types::shapes::*;
extern crate rmp_serde as rmps;
use rand::*;

use crate::util::*;
use crate::types::*;

use sc_types::constants::*;

use crate::render::Renderer;

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

fn apply_updates(game_state: &mut GameState, updates: [&Vec<GameCommand>; 2], p_id: usize, frame: i32) {
    for i in 0..=1 {
        for u in updates[i] {
            let units = if p_id == i { &mut game_state.my_units } else { &mut game_state.other_units };
            match u {
                GameCommand::Blink(BlinkCommand { u_id }) => {
                    if *u_id < units.len() {
                        units[*u_id].blink_cooldown = units[*u_id].cooldown();
                        units[*u_id].blinking = Some(true);
                    }
                },
                GameCommand::Spawn(SpawnMsgCommand { path, player_id }) => {
                    units.push(Unit {
                        dead: false,
                        player_id: *player_id,
                        pos: path[0],
                        path: path.clone(),
                        blinking: None,
                        blink_cooldown: 0,
                        carrying_bounty: HashMap::new(),
                    });
                    game_state.spawn_cooldown[*player_id] = MSG_COOLDOWN;
                    game_state.lumber[*player_id] -= max(0, path_lumber_cost(path) - MSG_FREE_LUMBER);
                },
                GameCommand::Intercept(InterceptCommand { pos }) => {
                    game_state.interceptions.push(Interception { pos: pos.clone(), start_frame: frame, player_id: i });
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

    for intercept in &mut game_state.interceptions {
        if frame - intercept.start_frame >= INTERCEPT_DELAY as i32 {
            let other_units = if p_id == intercept.player_id { &mut game_state.other_units } else { &mut game_state.my_units };
            for unit in other_units.iter_mut() {
                // Have to check unit.dead to avoid double counting interception kills (If 2 interceptions kill the same unit on the same frame)
                if !unit.dead {
                    if rounded(unit.pos) == intercept.pos {
                        unit.dead = true;
                        game_state.intercepted[intercept.player_id] += 1;
                    }
                }
            }
        }
    }
    game_state.interceptions.retain(|i| (frame - i.start_frame) < INTERCEPT_EXPIRY + INTERCEPT_DELAY);
    reap(game_state);
    game_state.other_units.retain(|u| !u.dead);
}

fn apply_bounties(game_state: &mut GameState, p_id: usize, bounties: HashMap<BountyEnum, i32>) {
    for (b_type, amt) in bounties.iter() {
        match *b_type {
            BountyEnum::Fuel => { game_state.fuel[p_id] += *amt },
            BountyEnum::Gold => { game_state.gold[p_id] += *amt as f32 },
            BountyEnum::Lumber => { game_state.lumber[p_id] += *amt },
            _ => {}
        }
    }
}

fn same_tile(a: Vector2, b: Vector2) -> bool {
    a.x.round() == b.x.round() && a.y.round() == b.y.round()
}

fn deliver_messages(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    let my_bounties = game_state.my_units.iter_mut().filter(|u| station(u.player_id).iter().any(|s| same_tile(u.pos, *s)))
        .map(|u| { u.dead = true; u }).fold(HashMap::new(), |acc, e| hm_add(acc, &e.carrying_bounty));
    apply_bounties(game_state, p_id, my_bounties);
    reap(game_state);
    let other_bounties = game_state.other_units.iter_mut().filter(|u| station(u.player_id).iter().any(|s| same_tile(u.pos, *s)))
        .map(|u| { u.dead = true; u }).fold(HashMap::new(), |acc, e| hm_add(acc, &e.carrying_bounty));
    apply_bounties(game_state, other_id, other_bounties);
    game_state.other_units.retain(|u| !u.dead);

    game_state.fuel[p_id] = min(START_FUEL, game_state.fuel[p_id] + (num_my_units - game_state.my_units.len() as i32) * MSG_FUEL);
    game_state.fuel[other_id] = min(START_FUEL, game_state.fuel[other_id] + (num_other_units - game_state.other_units.len() as i32) * MSG_FUEL);

    game_state.gold[p_id] += (num_my_units - game_state.my_units.len() as i32) as f32 * MSG_DELIVERY_GOLD_BOUNTY;
    game_state.gold[other_id] += (num_other_units - game_state.other_units.len() as i32) as f32 * MSG_DELIVERY_GOLD_BOUNTY;
}

fn tick(game_state: &mut GameState) {
    for u in game_state.my_units.iter_mut().chain(game_state.other_units.iter_mut()) {
        u.blink_cooldown = max(0, u.blink_cooldown - 1);
    }

    game_state.fuel.iter_mut().for_each(|f| *f -= FUEL_LOSS);
    game_state.gold.iter_mut().for_each(|g| *g += PASSIVE_GOLD_GAIN);
    game_state.spawn_cooldown.iter_mut().for_each(|s| *s = max(*s - 1, 0));
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

fn no_hmap_units(units: &Vec<Unit>) -> Vec<Unit> {
    units.iter().map(|u| Unit { carrying_bounty: HashMap::new(), ..u.clone() }).collect()
}

fn serialize_state(game_state: &GameState, p_id: usize) -> Result<Vec<u8>, rmps::encode::Error> {
    let mut v;
    // FIXME serialize units.carrying_bounty
    if p_id == 0 {
        v = rmp_serde::encode::to_vec(&no_hmap_units(&game_state.my_units))?;
        v.append(&mut rmp_serde::encode::to_vec(&no_hmap_units(&game_state.other_units))?);
    } else {
        v = rmp_serde::encode::to_vec(&no_hmap_units(&game_state.other_units))?;
        v.append(&mut rmp_serde::encode::to_vec(&no_hmap_units(&game_state.my_units))?);
    }
    v.append(&mut rmp_serde::encode::to_vec(&game_state.fuel)?);
    v.append(&mut rmp_serde::encode::to_vec(&game_state.intercepted)?);
    v.append(&mut rmp_serde::encode::to_vec(&game_state.gold)?);
    // FIXME serialize upgrades and items correctly (easiest might be to convert to sorted vec and serialize)
    let upg: Vec<usize> = game_state.upgrades.iter().map(|hs| hs.len()).collect();
    v.append(&mut rmp_serde::encode::to_vec(&upg)?);
    v.append(&mut rmp_serde::encode::to_vec(&game_state.bounties)?);
    // FIXME serialize game_state.next_bounty
    Ok(v)
}

fn bounty_counts(bounties: &Vec<Bounty>) -> Vec<(BountyEnum, usize)> {
    let mut out = vec![];
    for b_type in [BountyEnum::Blink, BountyEnum::Fuel, BountyEnum::Gold, BountyEnum::Lumber] {
        out.push((b_type, bounties.iter().filter(|b| b.type_ == b_type).count()));
    }
    out
}

fn add_bounty(game_state: &mut GameState) {
    let rng = &mut game_state.rng;
    if game_state.spawn_bounties {
        let counts = bounty_counts(&game_state.bounties);
        let existing_dist: Vec<(BountyEnum, f32)> = if game_state.bounties.is_empty() {
                vec![(BountyEnum::Blink, 0.25), (BountyEnum::Fuel, 0.25), (BountyEnum::Lumber, 0.25), (BountyEnum::Gold, 0.25)]
            } else {
                counts.iter().map(|(k, v)| (*k, *v as f32/game_state.bounties.len() as f32)).collect()
            };
        let mut p_dist: Vec<(BountyEnum, f32)> = vec![];
        for (k, v) in existing_dist {
            p_dist.push((k, (1f32 - v)/3f32));
        }
        let r = rng.gen_range(0..100);
        let (m_t_to_spawn, _) = p_dist.iter().fold((None, r), |(m_out, acc_r), (b_type, p)| {
            match m_out {
                Some(out) => (Some(out), acc_r),
                None => {
                    if acc_r < (p * 100f32).round() as i32 {
                        (Some(*b_type), acc_r)
                    } else {
                        (None, acc_r - (p * 100f32).round() as i32)
                    }
                }
            }
        });

        let t_to_spawn = m_t_to_spawn.unwrap_or(p_dist[p_dist.len() - 1].0);

        let mut b = Vector2::new(rng.gen_range(PLAY_AREA.x..(PLAY_AREA.x + PLAY_AREA.w)) as f32, rng.gen_range(PLAY_AREA.y..(PLAY_AREA.y + PLAY_AREA.h)) as f32);
        while same_tile(*ship(0), b) ||
              same_tile(*ship(1), b) ||
              station(0).iter().any(|s| same_tile(*s, b)) ||
              station(1).iter().any(|s| same_tile(*s, b)) ||
                game_state.bounties.iter().any(|existing_b| same_tile(existing_b.pos, b)) {
            b = Vector2::new(rng.gen_range(PLAY_AREA.x..(PLAY_AREA.x + PLAY_AREA.w)) as f32, rng.gen_range(PLAY_AREA.y..(PLAY_AREA.y + PLAY_AREA.h)) as f32);
        }
        game_state.bounties.push(Bounty { type_: t_to_spawn, amount: t_to_spawn.amount(rng), pos: b });
    } 
}

fn collide_bounties(game_state: &mut GameState) {
    let pack_bounty = |m_unit: Option<&mut Unit>, b: &Bounty| {
        if let Some(unit) = m_unit {
            if b.type_ == BountyEnum::Blink {
                unit.blink_cooldown = 0;
                if unit.blinking.is_none() {
                    unit.blinking = Some(false);
                }
            } else {
                unit.carrying_bounty.entry(b.type_).and_modify(|e| *e += b.amount).or_insert(b.amount);
            }
        } 
    };

    for b in &game_state.bounties {
        let m_mine = game_state.my_units.iter_mut().find(|u| same_tile(u.pos, b.pos));
        let m_other = game_state.other_units.iter_mut().find(|u| same_tile(u.pos, b.pos));
        pack_bounty(m_mine, b);
        pack_bounty(m_other, b);
    }

    // PERF loop only once
    game_state.bounties.retain(|b| !game_state.my_units.iter().any(|u| same_tile(u.pos, b.pos)) &&
        !game_state.other_units.iter().any(|u| same_tile(u.pos, b.pos)))
}

fn path_lumber_cost(path: &VecDeque<Vector2>) -> i32 {
    if path.len() <= 1 {
        0
    } else {
        path.iter().skip(2).fold((0, path[1], (path[1] - path[0]).normalized()), |(acc, last, dir), e| {
            let new_dir = (*e - last).normalized();
            if new_dir == dir {
                (acc, *e, new_dir)
            } else {
                (acc + 1, *e, new_dir)
            }
        }).0
    }
}

pub fn set_non_fullscreen_window_size(rl: &mut RaylibHandle) {
    let mon_idx = get_current_monitor();
    let (mon_width, mon_height) = (get_monitor_width(mon_idx), get_monitor_height(mon_idx));
    if mon_width >= 3840 && mon_height >= 2160 {
        rl.set_window_size((3840 * 3)/4, (2160 * 3)/4);
    } else if mon_width >= 2560 && mon_height >= 1440 {
        rl.set_window_size((2560 * 3)/4, (1440 * 3)/4);
    } else if mon_width >= 1920 && mon_height >= 1080 {
        rl.set_window_size((1920 * 3)/4, (1080 * 3)/4);
    } else if mon_width >= 1366 && mon_height >= 768 {
        rl.set_window_size((1366 * 3)/4, (768 * 3)/4);
    } else if mon_width >= 1024 && mon_height >= 768 {
        rl.set_window_size((1024 * 3)/4, (576 * 3)/4);
    } else {
        rl.set_window_size(640, 360);
    }
    rl.set_window_position(mon_width/8, mon_height/8);
}

pub enum MouseState {
    Drag(Vector2),
    Path(VecDeque<Vector2>, bool),
    Intercept,
    WaitReleaseLButton,
    None
}

pub fn run_game(game_state: &mut GameState, screen_changed: &mut bool, zoom: &mut bool, borderless: &mut bool,
    rl: &mut RaylibHandle, mouse_state: &mut MouseState, net: &mut NetState,
    frame_counter: &mut i32, socket: &UdpSocket, server: &Vec<std::net::SocketAddr>, seq_state: &mut SeqState, frame_rate: u32,
    game_ps: &mut TimeWindowAvg) -> ClientState {
    let p_id = game_state.p_id;
    let raw_mouse_position = rl.get_mouse_position();
    let screen_width =  rl.get_screen_width() as f64;
    let screen_height = rl.get_screen_height() as f64;
    let mouse_position = Renderer::screen2world(raw_mouse_position, screen_width, screen_height, *zoom);
    let clip_mouse_position = Renderer::screen2clip(raw_mouse_position, screen_width, screen_height);
    let iso_proj = Renderer::iso_proj(screen_width, screen_height, *zoom);
    let mouse_tile = Vector2::new(mouse_position.x.round(), mouse_position.y.round());
    let mut start_message_path = false;
    let mut cancel = false;
    let mut start_intercept = false;
    *screen_changed = false;
    loop {
        match rl.get_key_pressed() {
            Some(k) => {
                match k {
                    KeyboardKey::KEY_P => {
                        *zoom = !*zoom;
                    },
                    KeyboardKey::KEY_ENTER => {
                        if rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT) {
                            let mon_idx = get_current_monitor();
                            if *borderless {
                                rl.toggle_borderless_windowed();
                                set_non_fullscreen_window_size(rl);
                                *borderless = false;
                            } else {
                                let (mon_width, mon_height) = (get_monitor_width(mon_idx), get_monitor_height(mon_idx));
                                rl.set_window_size(mon_width, mon_height);
                                rl.toggle_borderless_windowed();
                                *borderless = true;
                            }
                            *screen_changed = true;
                        }
                    }
                    KeyboardKey::KEY_ONE => {
                        game_state.selection = HashSet::new();
                        game_state.selection.insert(Selection::Ship);
                        game_state.sub_selection = Some(SubSelection::Ship);
                    },
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
                    KeyboardKey::KEY_Q => {
                        if game_state.spawn_cooldown[p_id] <= 0 {
                            start_message_path = true
                        }
                    },
                    KeyboardKey::KEY_W => {
                        if game_state.gold[p_id] < INTERCEPT_COST {
                            // TODO show ui report error
                        } else {
                            start_intercept = true;
                        }
                    },
                    KeyboardKey::KEY_ESCAPE => {
                        match mouse_state {
                            MouseState::Path(_, _) => { cancel = true }
                            MouseState::Intercept => { cancel = true }
                            _ => {}
                        }
                    },
                    KeyboardKey::KEY_Z => {
                        for (u_id, u) in selected_units(&game_state) {
                            if u.blink_cooldown <= 0 && u.blinking.is_some() {
                                net.queue_command(GameCommand::Blink(BlinkCommand { u_id }));
                            }
                        }
                    },
                    _ => {}
                }
            }
            None => break
        }
    }

    match mouse_state {
        MouseState::None => {
            if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                *mouse_state = MouseState::Drag(raw_mouse_position);
            } else if start_message_path {
                *mouse_state = MouseState::Path(VecDeque::from(vec![*ship(p_id)]), true);
            } else if start_intercept {
                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_CROSSHAIR);
                *mouse_state = MouseState::Intercept;
            } else {
                *mouse_state = MouseState::None;
            }
        },
        MouseState::Drag(start_pos) => {
            if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                *mouse_state = MouseState::Drag(*start_pos);
            } else {
                let start_pos_clip = Renderer::screen2clip(*start_pos, screen_width, screen_height);
                let selection_pos = Vector2 { x: start_pos_clip.x.min(clip_mouse_position.x), y: start_pos_clip.y.min(clip_mouse_position.y) };
                let selection_size = Vector2 { x: (start_pos_clip.x - clip_mouse_position.x).abs(), y: (start_pos_clip.y - clip_mouse_position.y).abs() };
                let selection_rect = Rect { x: selection_pos.x, y: selection_pos.y, w: selection_size.x, h: selection_size.y };

                // FIXME use cube_z_offset
                fn unit_vec4(v2: Vector2) -> Vector4 { Vector4::new(v2.x, v2.y, 0.5, 1.0) }
                fn unit_screen_pos(v4: Vector4) -> Vector2 { Vector2::new(v4.x, v4.y) }
                let in_box: Vec<_> = game_state.my_units.iter().enumerate().filter(|(_, u)| selection_rect.contains_point(&unit_screen_pos(unit_vec4(u.pos).transform(iso_proj)))).map(|(i, _)| Selection::Unit(i)).collect();
                if !in_box.is_empty() {
                    if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
                        game_state.selection = game_state.selection.symmetric_difference(&HashSet::from_iter(in_box)).cloned().collect();
                    } else {
                        game_state.selection = HashSet::from_iter(in_box);
                    }
                }
                if game_state.selection.iter().any(|s| if let Selection::Unit(_) = s { true } else { false }) {
                    game_state.sub_selection = Some(SubSelection::Unit);
                } else {
                    game_state.sub_selection = Some(SubSelection::Ship);
                }
                *mouse_state = MouseState::None;
            }
        },
        MouseState::Path(path, y_first) => {
            if cancel {
                *mouse_state = MouseState::None;
            } else {
                if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) {
                    *y_first = !*y_first;
                } else if PLAY_AREA.contains_point(&mouse_tile) && rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                    let p = path[path.len() - 1];
                    let m: Vector2;
                    if *y_first {
                        m = Vector2::new(p.x.round(), mouse_position.y.round());
                    } else {
                        m = Vector2::new(mouse_position.x.round(), p.y.round());
                    }
                    path.push_back(m);
                    if !(station(p_id).iter().any(|s| *s == m)) {
                        path.push_back(Vector2::new(mouse_position.x.round(), mouse_position.y.round()));
                    }
                    if  station(p_id).iter().any(|s| *s == m) ||
                        station(p_id).iter().any(|s| *s == Vector2::new(mouse_position.x.round(), mouse_position.y.round())) {
                        if game_state.lumber[p_id] >= path_lumber_cost(&path) - MSG_FREE_LUMBER {
                            net.queue_command(GameCommand::Spawn(SpawnMsgCommand { player_id: p_id, path: path.clone() }));
                            *mouse_state = MouseState::WaitReleaseLButton;
                        } else {
                            // TODO show ui error not enought lumber
                            *mouse_state = MouseState::WaitReleaseLButton;
                        }
                    }
                }
            }
        },
        MouseState::Intercept => {
            if cancel {
                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                *mouse_state = MouseState::None;
            } else if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                if PLAY_AREA.contains_point(&mouse_tile) &&
                        game_state.gold[p_id] >= INTERCEPT_COST {
                    net.queue_command(GameCommand::Intercept(InterceptCommand { pos: mouse_tile }));
                    rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                    *mouse_state = MouseState::WaitReleaseLButton;
                } else {
                    // TODO show error if not enough gold
                    *mouse_state = MouseState::Intercept;
                }
            } else {
                *mouse_state = MouseState::Intercept;
            }
        },
        MouseState::WaitReleaseLButton => {
            if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                *mouse_state = MouseState::WaitReleaseLButton;
            } else {
                *mouse_state = MouseState::None;
            }
        }
    };

    // TODO use types to make sure sent/recvd packet can't be mistaken for each other
    if let Some((sent_pkt, recvd_pkt)) = net.process(*frame_counter, &socket, &server, seq_state, frame_rate) {
        if game_state.bounties.len() >= 10 {
            game_state.spawn_bounties = false;
        }
        if game_state.bounties.len() < 6 {
            game_state.spawn_bounties = true;
        }
        game_ps.sample();
        apply_updates(game_state, if p_id == 0 { [&sent_pkt, &recvd_pkt] } else { [&recvd_pkt, &sent_pkt] }, p_id, *frame_counter);
        
        if (*frame_counter % (3 * 60)) == 0 {
            add_bounty(game_state);
        }
        move_units(&mut game_state.my_units);
        move_units(&mut game_state.other_units);
        deliver_messages(game_state, p_id);
        collide_bounties(game_state);
        tick(game_state);
        *frame_counter += 1;
        if *frame_counter % 60 == 0 {
            socket_send(&socket, &server[0], &ClientPkt::StateHash { 
                seq: seq_state.send_seq,
                ack: seq_state.send_ack,
                hash: crc32fast::hash(&serialize_state(&game_state, p_id).unwrap()),
                frame: *frame_counter,
            }).unwrap();
            seq_state.send();
        }
    }

    if game_state.fuel.iter().any(|f| *f <= 0) || game_state.intercepted.iter().any(|v| *v >= KILLS_TO_WIN) {
        socket_send(&socket, &server[0], &ClientPkt::Ended { 
            seq: seq_state.send_seq,
            ack: seq_state.send_ack,
            frame: *frame_counter,
        }).unwrap();
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
}