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
mod render;

use util::*;
use sc_types::constants::*;
use types::*;

use crate::render::Renderer;
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

fn same_tile(a: Vector2, b: Vector2) -> bool {
    a.x.round() == b.x.round() && a.y.round() == b.y.round()
}

fn deliver_messages(game_state: &mut GameState, p_id: usize) {
    let other_id = (p_id + 1) % 2;

    let num_my_units = game_state.my_units.len() as i32;
    let num_other_units = game_state.other_units.len() as i32;

    let my_bounties = game_state.my_units.iter_mut().filter(|u| same_tile(u.pos, *station(u.player_id)))
        .map(|u| { u.dead = true; u }).fold(HashMap::new(), |acc, e| hm_add(acc, &e.carrying_bounty));
    apply_bounties(game_state, p_id, my_bounties);
    reap(game_state);
    let other_bounties = game_state.other_units.iter_mut().filter(|u| same_tile(u.pos, *station(u.player_id)))
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

        let mut b = Vector2::new(rng.gen_range(PLAY_AREA.x..(PLAY_AREA.x + PLAY_AREA.w)) as f32, rng.gen_range(PLAY_AREA.y..(PLAY_AREA.y + PLAY_AREA.h)) as f32);
        while same_tile(*ship(0), b) ||
              same_tile(*ship(1), b) ||
              same_tile(*station(0), b) ||
              same_tile(*station(1), b) ||
                game_state.bounties.iter().any(|existing_b| same_tile(existing_b.pos, b)) {
            b = Vector2::new(rng.gen_range(PLAY_AREA.x..(PLAY_AREA.x + PLAY_AREA.w)) as f32, rng.gen_range(PLAY_AREA.y..(PLAY_AREA.y + PLAY_AREA.h)) as f32);
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
        let m_mine = game_state.my_units.iter_mut().find(|u| same_tile(u.pos, b.pos));
        let m_other = game_state.other_units.iter_mut().find(|u| same_tile(u.pos, b.pos));
        pack_bounty(m_mine, b);
        pack_bounty(m_other, b);
    }

    // PERF loop only once
    game_state.bounties.retain(|b| !game_state.my_units.iter().any(|u| same_tile(u.pos, b.pos)) &&
        !game_state.other_units.iter().any(|u| same_tile(u.pos, b.pos)))
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

pub enum MouseState {
    Drag(Vector2),
    Path(VecDeque<Vector2>, bool),
    Intercept(bool),
    WaitReleaseLButton,
    None
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

    let (mut rl, thread) = raylib::init()
        .size(2560, 1440)
        // .fullscreen()
        .title("Space Codes")
        .msaa_4x()
        .build();
    // rl.set_trace_log(TraceLogLevel::LOG_ERROR);
    rl.set_target_fps(frame_rate);

    let mut render = Renderer::new(&mut rl, &thread);

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
    let mut unacked_pkts: FrameMap<Vec<GameCommand>> = FrameMap::new();
    let mut future_pkts: FrameMap<Vec<GameCommand>> = FrameMap::new();
    let mut sent_pkts: FrameMap<Vec<GameCommand>> = FrameMap::new();
    let mut last_rcvd_pkt = -1;
    let mut my_frame_delay = 1u8;
    let mut m_new_frame_delay = None;
    // ------------------------
    let mut interceptions = vec![];
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
    let mut waiting = Instant::now();
    let mut waiting_avg = WindowAvg::new(frame_rate as usize * 10);

    fn vec2(v3: Vector3) -> Vector2 {
        Vector2::new(v3.x, v3.y)
    }
    let start_time = Instant::now();
    let mut zoom = false;
    while !rl.window_should_close() {
        let raw_mouse_position = rl.get_mouse_position();
        let screen_width =  rl.get_screen_width() as f64;
        let screen_height = rl.get_screen_height() as f64;
        let mouse_position = Renderer::screen2world(raw_mouse_position, screen_width, screen_height, zoom);
        let clip_mouse_position = Renderer::screen2clip(raw_mouse_position, screen_width, screen_height);
        let iso_proj = Renderer::iso_proj(screen_width, screen_height, zoom);
        let rounded_mouse_pos = Vector2::new(mouse_position.x.round(), mouse_position.y.round());

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
                        unacked_pkts = FrameMap::new();
                        future_pkts = FrameMap::new();
                        sent_pkts = FrameMap::new();
                        my_frame_delay = 1;
                        m_new_frame_delay = None;
                        waiting = Instant::now();
                        for i in 0..my_frame_delay {
                            future_pkts.push(i as i64, vec![]);
                            sent_pkts.push(i as i64, vec![]);                            
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
                if game_state.bounties.len() >= 10 {
                    game_state.spawn_bounties = false;
                }
                if game_state.bounties.len() < 6 {
                    game_state.spawn_bounties = true;
                }

                let resp = socket_recv(&socket, &server[0], &mut seq_state);
                match resp {
                    None => {}
                    Some(ServerEnum::UpdateOtherTarget { updates, frame, frame_ack, frame_delay }) => {
                        waiting_avg.sample(waiting.elapsed().as_secs_f64());
                        waiting = Instant::now();
                        future_pkts.merge(&updates.clone());
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
                                    if game_state.spawn_cooldown[p_id] <= 0 {
                                        start_message_path = true
                                    }
                                },
                                KeyboardKey::KEY_P => {
                                    zoom = !zoom;
                                }
                                KeyboardKey::KEY_W => {
                                    if game_state.gold[p_id] < INTERCEPT_COST {
                                        intercept_err = true;
                                    } else {
                                        start_intercept = true;
                                    }
                                }
                                KeyboardKey::KEY_S => { shop_open = !shop_open }
                                KeyboardKey::KEY_ESCAPE => {
                                    match mouse_state {
                                        MouseState::Path(_, _) => { cancel = true }
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

                not_enough_lumber = false;
                mouse_state = match mouse_state {
                    MouseState::None => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                            MouseState::Drag(raw_mouse_position)
                        } else if start_message_path {
                            MouseState::Path(VecDeque::from(vec![*ship(p_id)]), true)
                        } else if start_intercept {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_POINTING_HAND);
                            MouseState::Intercept(false)
                        } else {
                            MouseState::None
                        }
                    },
                    MouseState::Drag(start_pos) => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                            MouseState::Drag(start_pos)
                        } else {
                            let start_pos_clip = Renderer::screen2clip(start_pos, screen_width, screen_height);
                            let selection_pos = Vector2 { x: start_pos_clip.x.min(clip_mouse_position.x), y: start_pos_clip.y.min(clip_mouse_position.y) };
                            let selection_size = Vector2 { x: (start_pos_clip.x - clip_mouse_position.x).abs(), y: (start_pos_clip.y - clip_mouse_position.y).abs() };
                            let selection_rect = Rect { x: selection_pos.x, y: selection_pos.y, w: selection_size.x, h: selection_size.y };

                            // FIXME use cube_z_offset
                            fn unit_vec4(v2: Vector2) -> Vector4 { Vector4::new(v2.x, v2.y, 0.5, 1.0) }
                            fn unit_screen_pos(v4: Vector4) -> Vector2 { Vector2::new(v4.x, v4.y) };
                            let mut in_box: Vec<_> = game_state.my_units.iter().enumerate().filter(|(_, u)| selection_rect.contains_point(&unit_screen_pos(unit_vec4(u.pos).transform(iso_proj)))).map(|(i, _)| Selection::Unit(i)).collect();
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
                            MouseState::None
                        }
                    },
                    MouseState::Path(mut path, y_first) => {
                        if cancel {
                            MouseState::None
                        } else {
                            if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) {
                                MouseState::Path(path, !y_first)
                            } else if PLAY_AREA.contains_point(&rounded_mouse_pos) && rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                                let p = path[path.len() - 1];
                                let m: Vector2;
                                if y_first {
                                    m = Vector2::new(p.x.round(), mouse_position.y.round());
                                } else {
                                    m = Vector2::new(mouse_position.x.round(), p.y.round());
                                }
                                path.push_back(m);
                                if !(*station(p_id) == m) {
                                    path.push_back(Vector2::new(mouse_position.x.round(), mouse_position.y.round()));
                                }
                                if *station(p_id) == m || *station(p_id) == Vector2::new(mouse_position.x.round(), mouse_position.y.round()) {
                                    if game_state.lumber[p_id] >= path_lumber_cost(&path) - MSG_FREE_LUMBER {
                                        unsent_pkt.push(GameCommand::Spawn(SpawnMsgCommand { player_id: p_id, path: path.clone() }));
                                        MouseState::WaitReleaseLButton
                                    } else {
                                        not_enough_lumber = true;
                                        MouseState::WaitReleaseLButton
                                    }
                                } else {
                                    MouseState::Path(path, y_first)
                                }
                            } else {
                                MouseState::Path(path, y_first)
                            }
                        }
                    },
                    MouseState::Intercept(vertical) => {
                        if cancel {
                            rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                            MouseState::None
                        } else if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                            if PLAY_AREA.contains_point(&vec2(mouse_position)) &&
                                    !game_state.other_units.iter().any(|other_u| intercept_inside_bubble(other_u, &Interception { start_frame: 0, pos: vec2(mouse_position), vertical: vertical, player_id: 0 })) &&
                                    game_state.gold[p_id] >= INTERCEPT_COST {
                                unsent_pkt.push(GameCommand::Intercept(InterceptCommand { pos: vec2(mouse_position), vertical: vertical }));
                                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
                                MouseState::WaitReleaseLButton
                            } else {
                                intercept_err = true;
                                MouseState::Intercept(vertical)
                            }
                        } else if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) {
                            MouseState::Intercept(!vertical)
                        } else {
                            MouseState::Intercept(vertical)
                        }
                    },
                    MouseState::WaitReleaseLButton => {
                        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                            MouseState::WaitReleaseLButton
                        } else {
                            MouseState::None
                        }
                    }
                };

                if next_send_frame <= frame_counter {
                    let mut dont_send = false;
                    if let Some(new_frame_delay) = m_new_frame_delay {
                        if new_frame_delay > my_frame_delay {
                            for i in my_frame_delay..new_frame_delay {
                                unacked_pkts.push(frame_counter + i as i64, vec![]);
                                sent_pkts.push(frame_counter + i as i64, vec![]);
                            }
                            m_new_frame_delay = None;
                            my_frame_delay = new_frame_delay;
                        } else {
                            if sent_pkts.iter().any(|(f, _)| *f >= frame_counter + new_frame_delay as i64) {
                                dont_send = true;
                            } else {
                                m_new_frame_delay = None;
                                my_frame_delay = new_frame_delay;
                            }
                        }
                    }
                    if !dont_send {
                        unacked_pkts.push(frame_counter + my_frame_delay as i64, unsent_pkt.clone());
                        socket_send(&socket, &server[0], &ClientPkt::Target { 
                            seq: seq_state.send_seq,
                            ack: seq_state.send_ack,
                            updates: unacked_pkts.cloned_vecdeque(),
                            frame: frame_counter + my_frame_delay as i64,
                            frame_ack: last_rcvd_pkt,
                            frame_delay: my_frame_delay
                        })?;
                        seq_state.send();
                        sent_pkts.push(frame_counter + my_frame_delay as i64, unsent_pkt.clone());
                        unsent_pkt = vec![];
                    }
                    next_send_frame += 1;
                }

                if (next_send_frame > frame_counter) && future_pkts.iter().any(|ps| ps.0 == frame_counter) {
                    game_ps.sample();
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

                let waiting_one_pct_max = f64::min(waiting_avg.one_percent_max(), 300f64/1000f64);
                if m_new_frame_delay.is_none() {
                    let new_delay = (waiting_one_pct_max * (frame_rate as f64)).ceil() as i32;
                    let mfd = my_frame_delay as i32;
                    if new_delay > mfd || new_delay < mfd/2 {
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

        render.render(&mut rl, &thread, frame_counter, p_id, &game_state, mouse_position, &mouse_state, &state, zoom, &NetInfo { game_ps: &game_ps, waiting_avg: &waiting_avg, my_frame_delay });
    }
    Ok(())
}