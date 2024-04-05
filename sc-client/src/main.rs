use std::collections::{HashMap, HashSet, VecDeque};
use std::cmp::{min, max};
use std::f32::EPSILON;
use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use std::time::Instant;
use pathfinding::path_collides;
use rand_core::SeedableRng;
use raylib::prelude::*;
use sc_types::*;
use sc_types::shapes::*;
extern crate rmp_serde as rmps;
use rand_chacha::*;
use rand::*;

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
    vertical: bool,
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
                GameCommand::Intercept(InterceptCommand { pos, vertical }) => {
                    interceptions.push(Interception { pos: pos.clone(), vertical: *vertical, start_frame: frame, player_id: i });
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
        let other_units = if p_id == intercept.player_id { &mut game_state.other_units } else { &mut game_state.my_units };
        for unit in other_units.iter_mut() {
            // Have to check unit.dead to avoid double counting interception kills (If 2 interceptions kill the same unit on the same frame)
            if !unit.dead {
                let int_ = intercept_line(intercept);
                if unit.rect().collide_line(&int_[0], &int_[1]) {
                    unit.dead = true;
                    game_state.intercepted[intercept.player_id] += 1;
                }
            }
        }
    }
    interceptions.retain(|i| ((frame - i.start_frame) as f32) < INTERCEPT_EXPIRY);
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

fn deliver_messages(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    let my_bounties = game_state.my_units.iter_mut().filter(|u| u.rect().collide(station(u.player_id)))
        .map(|u| { u.dead = true; u }).fold(HashMap::new(), |acc, e| hm_add(acc, &e.carrying_bounty));
    apply_bounties(game_state, p_id, my_bounties);
    reap(game_state);
    let other_bounties = game_state.other_units.iter_mut().filter(|u| u.rect().collide(station(u.player_id)))
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
    game_state.spawn_cooldown.iter_mut().for_each(|s| *s -= 1);
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

fn get_manhattan_turn_point(p1: Vector2, p2: Vector2, p_id: usize) -> (bool, Vector2) {
    let m1 = Vector2 { x: p1.x, y: p2.y };
    let m2 = Vector2 { x: p2.x, y: p1.y };
    let sx = Vector2 { x: MESSAGE_SIZE.x, y: 0f32 };
    let sy = Vector2 { x: 0f32, y: MESSAGE_SIZE.y };
    let offsets = [ Vector2::zero(), sx, sy, *MESSAGE_SIZE ];
    let mut blocked: Vec<Rect<i32>> = (if p_id == 0 { P0_BLOCKED } else { P1_BLOCKED }).to_vec();
    blocked.extend(BLOCKED.to_vec());
    let m1_ok = !path_collides(&blocked, offsets, p1, m1) && !path_collides(&blocked, offsets, m1, p2);
    let m2_ok = !path_collides(&blocked, offsets, p1, m2) && !path_collides(&blocked, offsets, m2, p2);
    let p1_ok = PLAY_AREA.contains(&unit_rect(&p1, MESSAGE_SIZE));
    let p2_ok = PLAY_AREA.contains(&unit_rect(&p2, MESSAGE_SIZE));
    let none = if (p1.x - p2.x).abs() < (p1.y - p2.y).abs() {
            (false, m1)
        } else {
            (false, m2)
        };
    if !p1_ok || !p2_ok {
        none
    } else {
        if m1_ok && m2_ok {
            if (p1.x - p2.x).abs() < (p1.y - p2.y).abs() {
                (true, m1)
            } else {
                (true, m2)
            }
        } else if m1_ok {
            (true, m1)
        } else if m2_ok {
            (true, m2)
        } else {
            none
        }
    }
}

fn bounty_counts(bounties: &Vec<Bounty>) -> Vec<(BountyEnum, usize)> {
    let mut out = vec![];
    for b_type in [BountyEnum::Blink, BountyEnum::Fuel, BountyEnum::Gold, BountyEnum::Lumber] {
        out.push((b_type, bounties.iter().filter(|b| b.type_ == b_type).count()));
    }
    out
}

fn add_bounty(game_state: &mut GameState, rng: &mut ChaCha20Rng) {
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

        let mut b = Vector2::new(rng.gen_range(PLAY_AREA.x..PLAY_AREA.w) as f32, rng.gen_range(PLAY_AREA.x..PLAY_AREA.h) as f32);
        while !PLAY_AREA.contains(&bounty_rect(&b)) ||
                GAME_MAP.iter().any(|r| r.1.collide(&bounty_rect(&b)) ||
                BLOCKED.iter().any(|r| r.collide(&bounty_rect(&b)))) ||
                (ship(0).center() - b).length() < 150f32 ||
                (ship(1).center() - b).length() < 150f32 ||
                game_state.bounties.iter().any(|existing_b| bounty_rect(&existing_b.pos).collide(&bounty_rect(&b))) {
            b = Vector2::new(rng.gen_range(PLAY_AREA.x..PLAY_AREA.w) as f32, rng.gen_range(PLAY_AREA.x..PLAY_AREA.h) as f32);
        }
        game_state.bounties.push(Bounty { type_: t_to_spawn, amount: t_to_spawn.amount(rng), pos: b });
    } 
}

fn bounty_rect(b: &Vector2) -> Rect<i32> {
    Rect { x: b.x.round() as i32, y: b.y.round() as i32, w: BOUNTY_SIZE.x.round() as i32, h: BOUNTY_SIZE.y.round() as i32 }
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
        let m_mine = game_state.my_units.iter_mut().find(|u| u.rect().collide(&bounty_rect(&b.pos)));
        let m_other = game_state.other_units.iter_mut().find(|u| u.rect().collide(&bounty_rect(&b.pos)));
        pack_bounty(m_mine, b);
        pack_bounty(m_other, b);
    }

    // PERF loop only once
    game_state.bounties.retain(|b| !game_state.my_units.iter().any(|u| u.rect().collide(&bounty_rect(&b.pos))) &&
        !game_state.other_units.iter().any(|u| u.rect().collide(&bounty_rect(&b.pos))))
}

fn intercept_line(intercept: &Interception) -> [Vector2; 2] {
    let p1: Vector2;
    let p2: Vector2;
    if intercept.vertical {
        p1 = intercept.pos - Vector2::new(0f32, INTERCEPT_LENGTH/2f32);
        p2 = intercept.pos + Vector2::new(0f32, INTERCEPT_LENGTH/2f32);
    } else {
        p1 = intercept.pos - Vector2::new(INTERCEPT_LENGTH/2f32, 0f32);
        p2 = intercept.pos + Vector2::new(INTERCEPT_LENGTH/2f32, 0f32);
    }
    [p1, p2]
}

fn bubble_rect(u: &Unit) -> Rect<i32> {
    let cen = u.pos + u.size().scale_by(0.5f32);
    let dir = (u.path[0] - u.pos).normalized();
    let bubble_pos: Vector2;
    let bubble_size: Vector2;
    if dir.x < -EPSILON { // left
        bubble_pos = cen + dir.scale_by(MSG_BUBBLE_LEN) + Vector2::new(0f32, -MSG_BUBBLE_WIDTH/2f32);
        bubble_size = Vector2::new(MSG_BUBBLE_LEN, MSG_BUBBLE_WIDTH);
    } else if dir.x > EPSILON { // right
        bubble_pos = cen + Vector2::new(0f32, -MSG_BUBBLE_WIDTH/2f32);
        bubble_size = Vector2::new(MSG_BUBBLE_LEN, MSG_BUBBLE_WIDTH);
    } else if dir.y < -EPSILON { // top
        bubble_pos = cen + dir.scale_by(MSG_BUBBLE_LEN) + Vector2::new(-MSG_BUBBLE_WIDTH/2f32, 0f32);
        bubble_size = Vector2::new(MSG_BUBBLE_WIDTH, MSG_BUBBLE_LEN);
    } else { // down
        bubble_pos = cen + Vector2::new(-MSG_BUBBLE_WIDTH/2f32, 0f32);
        bubble_size = Vector2::new(MSG_BUBBLE_WIDTH, MSG_BUBBLE_LEN);
    }
    Rect {
        x: bubble_pos.x.round() as i32,
        y: bubble_pos.y.round() as i32,
        w: bubble_size.x.round() as i32,
        h: bubble_size.y.round() as i32
    }
}

fn intercept_inside_bubble(u: &Unit, intercept: &Interception) -> bool {
    let int_ = intercept_line(intercept);
    bubble_rect(u).collide_line(&int_[0], &int_[1]) ||
        u.rect().collide_line(&int_[0], &int_[1])
}

fn draw_bubble(d: &mut RaylibDrawHandle, u: &Unit, c: &Color) {
    let b = bubble_rect(u);
    d.draw_rectangle_lines(b.x, b.y, b.w, b.h, c)
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

fn main() -> std::io::Result<()> {
    let frame_rate = 60;
    let max_input_queue = 10;
    let area_colors = HashMap::from([
        (AreaEnum::P0Spawn, rcolor(0, 0x77, 0xb6, 100)),
        (AreaEnum::P0Station, Color::from_hex("0077B6").unwrap()),
        (AreaEnum::P1Spawn, rcolor(0x1b, 0x43, 0x32, 100)),
        (AreaEnum::P1Station, Color::from_hex("1B4332").unwrap()),
        (AreaEnum::Blocked, Color::from_hex("99a3a3").unwrap()), // 4d908e 3d348b 33415c 
    ]);
    let intercept_colors = [rcolor(0x90, 0xE0, 0xEF, 255), rcolor(0x74, 0xC6, 0x9D, 255)];
    let msg_spawn_pos = [Vector2 { x: 490f32, y: 340f32 }, Vector2 { x: 340f32, y: 490f32 }];

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

    // set_trace_log(TraceLogLevel::LOG_ERROR);
    let (mut rl, thread) = raylib::init()
        .size(PLAY_AREA.w, PLAY_AREA.h + 200)
        .title("Space Codes")
        .vsync()
        .build();
    rl.set_target_fps(frame_rate);
    let mut blink_shader = rl.load_shader(&thread, Some("sc-client/src/vertex.vs"), Some("sc-client/src/shiny_blink.fs")).unwrap();
    let mut noise_shader = rl.load_shader(&thread, Some("sc-client/src/vertex.vs"), Some("sc-client/src/noise.fs")).unwrap();
    let u_time = blink_shader.get_shader_location("u_time");
    let u_resolution = blink_shader.get_shader_location("u_resolution");
    let u_blink_band = blink_shader.get_shader_location("u_blink_band");
    let u_top_left = blink_shader.get_shader_location("u_top_left");


    let message_spell_icons = MessageSpellIcons::new(&mut rl, &thread);
    let ship_spell_icons = ShipSpellIcons::new(&mut rl, &thread);
    let bounty_icons = Bounties::new(&mut rl, &thread);

    let mut state = ClientState::SendHello;
    // Most of these values doesn't matter. Its just for the compiler. They are initialized in ClientState::Waiting
    let mut game_state: GameState = GameState {
        my_units: vec![],
        other_units: vec![],
        selection: HashSet::new(),
        sub_selection: None,
        fuel: [START_FUEL; 2],
        intercepted: [0; 2],
        gold: [STARTING_GOLD; 2],
        lumber: [STARTING_LUMBER; 2],
        upgrades: [HashSet::new(), HashSet::new()],
        items: [HashMap::new(), HashMap::new()],
        bounties: vec![],
        spawn_bounties: true,
        last_bounty: HashMap::new(),
        spawn_cooldown: [0; 2],
    };
    let mut p_id = 0usize;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    // TODO All this netcode related stuff should be abstracted into a single type
    let mut next_send_frame = 0;
    let mut unsent_pkt = vec![];
    let mut unacked_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut future_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut sent_pkts: VecDeque<(i64, Vec<GameCommand>)> = VecDeque::new();
    let mut last_rcvd_pkt = -1;
    let mut my_frame_delay = 1u8;
    let mut m_new_frame_delay = None;
    // ------------------------
    let mut interceptions = vec![];
    enum MouseState {
        Drag(Vector2),
        Path(VecDeque<Vector2>),
        Intercept(bool),
        WaitReleaseLButton,
        None
    }
    let mut mouse_state: MouseState = MouseState::None;
    let mut ended = None;
    let mut game_ps = TimeWindowAvg::new();
    let mut shop_open = false;
    let mut rng: ChaCha20Rng = ChaCha20Rng::from_seed([0; 32]);
    let shop = Shop::new(&mut rl, &thread).unwrap();

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    rl.set_exit_key(None);
    let mut intercept_err = false;
    let mut not_enough_lumber = false;
    let mut waiting = None;
    let mut waiting_avg = WindowAvg::new();

    let start_time = Instant::now();
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
                    Some(ServerEnum::Start { rng_seed }) => {
                        frame_counter = 0;
                        next_send_frame = 0;
                        unsent_pkt = vec![];
                        unacked_pkts = VecDeque::new();
                        future_pkts = VecDeque::new();
                        sent_pkts = VecDeque::new();
                        my_frame_delay = 1;
                        m_new_frame_delay = None;
                        for i in 0..my_frame_delay {
                            future_pkts.push_front((i as i64, vec![]));
                            sent_pkts.push_front((i as i64, vec![]));                            
                        }
                        last_rcvd_pkt = -1;
                        ended = None;
                        interceptions = vec![];
                        mouse_state = MouseState::None;
                        shop_open = false;
                        rng = ChaCha20Rng::from_seed(rng_seed);
                        game_state = GameState {
                            my_units: vec![],
                            other_units: vec![],
                            selection: HashSet::from([Selection::Ship]),
                            sub_selection: Some(SubSelection::Ship),
                            fuel: [START_FUEL; 2],
                            intercepted: [0; 2],
                            gold: [STARTING_GOLD; 2],
                            lumber: [STARTING_LUMBER; 2],
                            upgrades: [HashSet::new(), HashSet::new()],
                            items: [HashMap::new(), HashMap::new()],
                            bounties: vec![],
                            spawn_bounties: true,
                            last_bounty: HashMap::from([
                                (BountyEnum::Blink, 0),
                                (BountyEnum::Fuel, 0),
                                (BountyEnum::Gold, 0),
                                (BountyEnum::Lumber, 0)
                            ]),
                            spawn_cooldown: [0; 2],
                        };
                        ClientState::Started
                    },
                    Some(_) => {
                        panic!("Expected Start")
                    }
                }
            },
            ClientState::Started => {
                if game_state.bounties.len() >= 25 {
                    game_state.spawn_bounties = false;
                }
                if game_state.bounties.len() < 15 {
                    game_state.spawn_bounties = true;
                }

                let resp = socket_recv(&socket, &server[0], &mut seq_state);
                match resp {
                    None => {}
                    Some(ServerEnum::UpdateOtherTarget { updates, frame, frame_ack, frame_delay }) => {
                        future_pkts.append(&mut updates.clone());
                        unacked_pkts.retain(|ps| ps.0 > frame_ack);
                        last_rcvd_pkt = frame;
                    },
                    Some(_) => {
                        panic!("Expected UpdateOtherTarget")
                    }
                }

                intercept_err = false;
                let mut start_message_path = false;
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
                                    if game_state.sub_selection == Some(SubSelection::Ship) && game_state.spawn_cooldown[p_id] <= 0 {
                                        start_message_path = true
                                    }
                                },
                                KeyboardKey::KEY_W => {
                                    if game_state.sub_selection == Some(SubSelection::Ship) {
                                        if game_state.gold[p_id] < INTERCEPT_COST {
                                            intercept_err = true;
                                        } else {
                                            start_intercept = true;
                                        }
                                    }
                                }
                                KeyboardKey::KEY_S => { shop_open = !shop_open }
                                KeyboardKey::KEY_ESCAPE => {
                                    match mouse_state {
                                        MouseState::Path(_) => { cancel = true }
                                        MouseState::Intercept(_) => { cancel = true }
                                        _ => {}
                                    }
                                },
                                KeyboardKey::KEY_Z => {
                                    for (u_id, u) in selected_units(&game_state) {
                                        if u.blink_cooldown <= 0 && u.blinking.is_some() {
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

                not_enough_lumber = false;
                mouse_state = match mouse_state {
                    MouseState::None => {
                        if PLAY_AREA.contains_point(&mouse_position) && rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            MouseState::Drag(mouse_position)
                        } else if start_message_path {
                            MouseState::Path(VecDeque::from(vec![msg_spawn_pos[p_id]]))
                        } else if start_intercept {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_POINTING_HAND);
                            MouseState::Intercept(false)
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
                            if !in_box.is_empty() {
                                if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT) {
                                    game_state.selection = game_state.selection.symmetric_difference(&HashSet::from_iter(in_box)).cloned().collect();
                                } else {
                                    game_state.selection = HashSet::from_iter(in_box);
                                }
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
                    MouseState::Path(mut path) => {
                        if cancel {
                            MouseState::None
                        } else {
                            if PLAY_AREA.contains_point(&mouse_position) && rl.is_mouse_button_pressed(MouseButton::MOUSE_LEFT_BUTTON) {
                                let eff_mouse_pos = mouse_position - MESSAGE_SIZE.scale_by(0.5f32);
                                if let (true, m) = get_manhattan_turn_point(path[path.len() - 1], eff_mouse_pos, p_id) {
                                    path.push_back(m);
                                    if !station(p_id).collide(&unit_rect(&m, MESSAGE_SIZE)) {
                                        path.push_back(eff_mouse_pos);
                                    }
                                    if station(p_id).collide(&unit_rect(&eff_mouse_pos, MESSAGE_SIZE)) ||
                                        station(p_id).collide(&unit_rect(&m, MESSAGE_SIZE)) {
                                        if game_state.lumber[p_id] >= path_lumber_cost(&path) - MSG_FREE_LUMBER {
                                            unsent_pkt.push(GameCommand::Spawn(SpawnMsgCommand { player_id: p_id, path: path.clone() }));
                                            MouseState::WaitReleaseLButton
                                        } else {
                                            not_enough_lumber = true;
                                            MouseState::WaitReleaseLButton
                                        }
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
                    },
                    MouseState::Intercept(vertical) => {
                        if cancel {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                            MouseState::None
                        } else if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                            if PLAY_AREA.contains_point(&mouse_position) &&
                                    !game_state.other_units.iter().any(|other_u| intercept_inside_bubble(other_u, &Interception { start_frame: 0, pos: mouse_position, vertical: vertical, player_id: 0 })) &&
                                    game_state.gold[p_id] >= INTERCEPT_COST {
                                unsent_pkt.push(GameCommand::Intercept(InterceptCommand { pos: mouse_position, vertical: vertical }));
                                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                                MouseState::WaitReleaseLButton
                            } else {
                                intercept_err = true;
                                MouseState::Intercept(vertical)
                            }
                        } else if rl.is_mouse_button_pressed(MouseButton::MOUSE_RIGHT_BUTTON) {
                            MouseState::Intercept(!vertical)
                        } else {
                            MouseState::Intercept(vertical)
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

                if next_send_frame <= frame_counter {
                    if let Some(new_frame_delay) = m_new_frame_delay {
                        for i in my_frame_delay..new_frame_delay {
                            unacked_pkts.push_front((frame_counter + i as i64, vec![]));
                            sent_pkts.push_front((frame_counter + i as i64, vec![]));
                        }
                        m_new_frame_delay = None;
                        my_frame_delay = new_frame_delay;
                    }
                    unacked_pkts.push_front((frame_counter + my_frame_delay as i64, unsent_pkt.clone()));
                    socket_send(&socket, &server[0], &ClientPkt::Target { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        updates: unacked_pkts.clone(),
                        frame: frame_counter + my_frame_delay as i64,
                        frame_ack: last_rcvd_pkt,
                        frame_delay: my_frame_delay
                    })?;
                    seq_state.send();
                    sent_pkts.push_front((frame_counter + my_frame_delay as i64, unsent_pkt.clone()));
                    unsent_pkt = vec![];
                    next_send_frame += 1;
                }

                if (next_send_frame > frame_counter) && !future_pkts.iter().any(|ps| ps.0 == frame_counter) {
                    match waiting {
                        None => waiting = Some(Instant::now()),
                        Some(wait) => {
                            if wait.elapsed().as_secs_f64() > 1f64 {
                                waiting_avg.sample(wait.elapsed().as_secs_f64());
                            }
                        }
                    }
                }

                if (next_send_frame > frame_counter) && future_pkts.iter().any(|ps| ps.0 == frame_counter) {
                    game_ps.sample();
                    match waiting {
                        Some(wait) => waiting_avg.sample(wait.elapsed().as_secs_f64()),
                        None => waiting_avg.sample(0f64)
                    };
                    waiting = None;
                    let recvd_pkt = future_pkts.iter().find(|ps| ps.0 == frame_counter).unwrap().1.clone();
                    let sent_pkt = sent_pkts.iter().find(|ps| ps.0 == frame_counter).unwrap().1.clone();
                    apply_updates(&mut game_state, if p_id == 0 { [&sent_pkt, &recvd_pkt] } else { [&recvd_pkt, &sent_pkt] }, p_id, &mut interceptions, frame_counter);
                    future_pkts.retain(|ps| ps.0 > frame_counter);
                    sent_pkts.retain(|ps| ps.0 > frame_counter);
                    if (frame_counter % (3 * 60)) == 0 {
                        add_bounty(&mut game_state, &mut rng);
                    }
                    move_units(&mut game_state.my_units);
                    move_units(&mut game_state.other_units);
                    deliver_messages(&mut game_state, p_id);
                    collide_bounties(&mut game_state);
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

                if waiting_avg.avg > 20f64/1000f64 && waiting_avg.avg < 300f64/1000f64 {
                    let new_delay = my_frame_delay as u32 + (waiting_avg.avg * (fps as f64)).ceil() as u32;
                    if new_delay < 20 {
                        m_new_frame_delay = Some(new_delay as u8);
                    }
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
                AreaEnum::P0Spawn => {
                    d.draw_rectangle(r.x, r.y, r.w, (r.h * game_state.spawn_cooldown[0])/MSG_COOLDOWN, area_colors[&t]);
                    d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]);
                }
                AreaEnum::P1Spawn => {
                    d.draw_rectangle(r.x, r.y, (r.w * game_state.spawn_cooldown[1])/MSG_COOLDOWN, r.h, area_colors[&t]);
                    d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]);
                }
                AreaEnum::P0Station => {
                    d.draw_rectangle(r.x, r.y, r.w, (r.h * game_state.fuel[0])/START_FUEL, area_colors[&t]);
                    d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]);
                }
                AreaEnum::P1Station => {
                    let w = (r.w * game_state.fuel[1])/START_FUEL;
                    d.draw_rectangle(r.x, r.y, w, r.h, area_colors[&t]);
                    d.draw_rectangle_lines(r.x, r.y, r.w, r.h, area_colors[&t]);
                }
                AreaEnum::Blocked => {
                    noise_shader.set_shader_value(noise_shader.get_shader_location("u_time"), start_time.elapsed().as_secs_f32());
                    let mut shd = d.begin_shader_mode(&noise_shader);
                    shd.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&t]);
                    drop(shd);
                }
            }
        }

        noise_shader.set_shader_value(noise_shader.get_shader_location("u_time"), start_time.elapsed().as_secs_f32());
        let mut shd = d.begin_shader_mode(&noise_shader);
        for r in &BLOCKED {
            shd.draw_rectangle(r.x, r.y, r.w, r.h, area_colors[&AreaEnum::Blocked])
        }
        drop(shd);

        d.draw_rectangle_lines(RIVER.x, RIVER.y, RIVER.w, RIVER.h, Color::BLUE);

        for u in game_state.my_units.iter().chain(game_state.other_units.iter()) {
            let c = if u.player_id == 0 { u.p0_colors() } else { u.p1_colors() };
            if u.blinking.is_some() {
                blink_shader.set_shader_value(u_time, start_time.elapsed().as_secs_f32());
                blink_shader.set_shader_value(u_resolution, PLAY_AREA.size() + Vector2::new(0f32, 200f32));
                blink_shader.set_shader_value(u_blink_band, Vector3::new(1f32, 1f32, 1f32));
                blink_shader.set_shader_value(u_top_left, u.pos);
                let mut shd = d.begin_shader_mode(&blink_shader);
                shd.draw_rectangle_v(u.pos, u.size(), c);
                drop(shd);
            } else {
                d.draw_rectangle_v(u.pos, u.size(), c);
            }

            let mx = Vector2::new(5f32, 0f32);
            let my = Vector2::new(0f32, 5f32);
            let ms = mx + my;
            if *u.carrying_bounty.get(&BountyEnum::Fuel).unwrap_or(&0) > 0 {
                d.draw_rectangle_v(u.pos + ms, ms, BountyEnum::Fuel.color());
            }
            if *u.carrying_bounty.get(&BountyEnum::Gold).unwrap_or(&0) > 0 {
                d.draw_rectangle_v(u.pos + ms + mx, ms, BountyEnum::Gold.color());
            }
            if *u.carrying_bounty.get(&BountyEnum::Lumber).unwrap_or(&0) > 0 {
                d.draw_rectangle_v(u.pos + ms + my, ms, BountyEnum::Lumber.color());
            }
            if u.blinking.is_some() {
                d.draw_rectangle_v(u.pos + ms + ms, ms, BountyEnum::Blink.color());
            }
            draw_bubble(&mut d, u, &c);
        }

        let mut sel_color = rcolor(0, 0, 0, 100);
        for s in &game_state.selection {
            match s {
                Selection::Unit(u_id) => {
                    if let Some(SubSelection::Unit) = game_state.sub_selection {
                        sel_color = rcolor(0, 0, 0, 150);
                    } else {
                        sel_color = rcolor(0, 0, 0, 100);
                    }
                    let u = &game_state.my_units[*u_id];
                    let selection_width = 4;
                    let rect = u.rect();
                    let r = Rectangle {
                        x: (rect.x - selection_width) as f32,
                        y: (rect.y - selection_width) as f32,
                        width: (rect.w + selection_width * 2) as f32,
                        height: (rect.h + selection_width * 2) as f32,
                    };
                    d.draw_rectangle_lines_ex(r, 4, sel_color);
                    if !u.path.is_empty() {
                        let mut p = u.pos + u.size().scale_by(0.5f32);
                        let col = rcolor(0, 255, 0, 100);
                        for i in 0..u.path.len() {
                            let next_p = u.path[i] + u.size().scale_by(0.5f32);
                            d.draw_line_ex(p, next_p, 2f32, col);
                            p = next_p;
                        }
                    }
                },
                Selection::Ship => {
                    if let Some(SubSelection::Ship) = game_state.sub_selection {
                        sel_color = rcolor(0, 0, 0, 150);
                    } else {
                        sel_color = rcolor(0, 0, 0, 100);
                    }
                    let selection_width = 4;
                    let rect = ship(p_id);
                    let r = Rectangle {
                        x: (rect.x - selection_width) as f32,
                        y: (rect.y - selection_width) as f32,
                        width: (rect.w + selection_width * 2) as f32,
                        height: (rect.h + selection_width * 2) as f32,
                    };
                    d.draw_rectangle_lines_ex(r, 4, sel_color);
                },
                Selection::Station => {
                    let rect = station(p_id);
                    d.draw_rectangle_lines(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2, sel_color)
                },
            }
        }   

        if game_state.sub_selection == Some(SubSelection::Unit) {
            let cooldowns: Vec<i32> =
                game_state.selection.iter().map(
                    |s| if let Selection::Unit(uid) = s {
                        let u = &game_state.my_units[*uid];
                        u.blinking.map(|_| game_state.my_units[*uid].blink_cooldown)
                    } else { None }
                ).flatten().collect();

            if !cooldowns.is_empty() {
                message_spell_icons.render(&mut d, (*cooldowns.iter().min().unwrap() as f32)/(BLINK_COOLDOWN as f32));
            }
        } else if game_state.sub_selection == Some(SubSelection::Ship) {
            ship_spell_icons.render(&mut d);
        }

        for a in &interceptions {
            let int_ = intercept_line(&a);
            d.draw_line_ex(int_[0], int_[1], 3f32, intercept_colors[a.player_id]);
        }

        if intercept_err || not_enough_lumber {
            d.draw_circle_v(mouse_position, 50f32, rcolor(255, 0, 0, 100));
        }

        for b in &game_state.bounties {
            bounty_icons.render(&mut d, b.type_, b.pos);
        }

        let path_width = 20f32;
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
                    d.draw_line_ex(p, next_p, path_width, col);
                    p = next_p;
                }

                let cost_pos = ship(p_id).center() - Vector2::new(5f32, 10f32);
                let mut cost = max(0, path_lumber_cost(path) - MSG_FREE_LUMBER);
                let eff_mouse_pos = mouse_position - MESSAGE_SIZE.scale_by(0.5f32);
                match get_manhattan_turn_point(p - MESSAGE_SIZE.scale_by(0.5f32), eff_mouse_pos, p_id) {
                    (true, m) => {
                        let mut tmp_path = path.clone();
                        tmp_path.push_back(m);
                        if !station(p_id).collide(&unit_rect(&m, MESSAGE_SIZE)) {
                            tmp_path.push_back(eff_mouse_pos);
                        }
                        cost = max(0, path_lumber_cost(&tmp_path) - MSG_FREE_LUMBER);
                        d.draw_line_ex(p, m + MESSAGE_SIZE.scale_by(0.5f32), path_width, col);
                        d.draw_line_ex(m + MESSAGE_SIZE.scale_by(0.5f32), mouse_position, path_width, col);
                    }
                    (false, m) => {
                        d.draw_line_ex(p, m + MESSAGE_SIZE.scale_by(0.5f32), path_width, bad_col);
                        d.draw_line_ex(m + MESSAGE_SIZE.scale_by(0.5f32), mouse_position, path_width, bad_col);
                    }
                }
                d.draw_text(&format!("{}", cost), cost_pos.x.round() as i32, cost_pos.y.round() as i32, 20, Color::BLACK);
            },
            MouseState::Intercept(vertical) => {
                let int_ = intercept_line(&Interception { start_frame: 0, player_id: 0, pos: mouse_position, vertical: vertical });
                d.draw_line_ex(int_[0], int_[1], 3f32, intercept_colors[p_id]);
            },
            _ => {}
        }

        d.draw_text(&format!("{:?}", state), 20, 20, 20, Color::BLACK);
        d.draw_text(&format!("fps/g: {}/{}", fps, game_ps.get_hz().round()), 20, 40, 20, Color::BLACK);
        d.draw_text(&format!("w/fd: {}/{}", (waiting_avg.avg * 1000f64).round(), my_frame_delay), 20, 60, 20, Color::BLACK);
        if let Some(end_state) = ended {
            let end_str = match end_state {
                Some(winner) => if winner == p_id { "YOU WON" } else { "YOU LOST" },
                None => "DRAW",
            };
            d.draw_text(&end_str, 470, 370, 20, Color::BLACK);
        }

        d.draw_line(0, PLAY_AREA.h, PLAY_AREA.w, PLAY_AREA.h, Color::BLACK);
        if let Some(sub_sel) = game_state.sub_selection {
            d.draw_text(&format!("Selected: {:?}", sub_sel), 20, PLAY_AREA.h, 20, Color::BLACK);
        }
        d.draw_text(&format!("Gold: {}/{}", game_state.gold[p_id].round(), game_state.gold[(p_id + 1) % 2].round()), 20, PLAY_AREA.h + 20, 20, Color::BLACK);
        d.draw_text(&format!("Lumber: {}/{}", game_state.lumber[p_id], game_state.lumber[(p_id + 1) % 2]), 20, PLAY_AREA.h + 40, 20, Color::BLACK);
        d.draw_text(&format!("Fuel: {}/{}", (game_state.fuel[p_id] * 100)/START_FUEL, (game_state.fuel[(p_id + 1) % 2] * 100)/START_FUEL), 20, PLAY_AREA.h + 60, 20, Color::BLACK);
        d.draw_text(&format!("K/D: {}/{}", game_state.intercepted[p_id], game_state.intercepted[(p_id + 1) % 2]), 20, PLAY_AREA.h + 80, 20, Color::BLACK);
        if shop_open {
            shop.render(&mut d, &game_state.upgrades[p_id], game_state.gold[p_id]);
        }
    }
    Ok(())
}