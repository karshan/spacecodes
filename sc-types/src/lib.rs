// extern crate serde;
extern crate serde_derive;

use std::{collections::{HashMap, HashSet, VecDeque}, hash::Hash};
use constants::{BLINK_COOLDOWN, MESSAGE_SIZE, MSG_FUEL};
use raylib::prelude::{Vector2, Color, rcolor};
use rand::Rng;
use rand_chacha::ChaCha20Rng;

pub mod shapes;
use serde::{Deserialize, Serialize};
// TODO enable with_serde feature on raylib then we don't need serde_nested or serde remote
use serde_nested_with::serde_nested;
use shapes::*;
pub mod constants;

#[derive(Default)]
pub struct SeqState {
    expected_seq: i32,
    expected_ack: i32,
    pub send_seq: i32,
    pub send_ack: i32,
}

impl SeqState {
    pub fn recv(&mut self, seq: i32, ack: i32) -> Option<String> {
        let mut e1 = None;
        let mut e2 = None;
        if ack > self.expected_ack {
            e1 = Some(format!("Expected ack {} got {}", self.expected_ack, ack))
        }

        if seq != self.expected_seq {
            e2 = Some(format!("Expected seq {} got {}", self.expected_seq, seq))
        }

        self.expected_seq = seq + 1;
        self.send_ack = seq;
        if let Some((mut m1, m2)) = e1.clone().zip(e2.clone()) {
            m1.push_str(&m2);
            Some(m1)
        } else {
            e1.or(e2)
        }
    }

    pub fn send(&mut self) {
        self.expected_ack = self.send_seq;
        self.send_seq = self.send_seq + 1;
    }
}

#[derive(Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Hash)]
pub enum Selection {
    Unit(usize),
    Station,
    Ship,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Hash)]
pub enum SubSelection {
    Unit,
    Station,
    Ship
}

pub enum ShopItem {
    Item(Item),
    Upgrade(Upgrade)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Upgrade {
    InterceptSpeed,
    InterceptRange
}

impl Upgrade {
    pub fn cost(self: &Self) -> f32 {
        match self {
            Upgrade::InterceptSpeed => 100f32,
            Upgrade::InterceptRange => 200f32,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Item {
    None
}

impl Item {
    pub fn cost(self: &Self) -> f32 {
        match self {
            Item::None => 0f32
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum BountyEnum {
    Gold,
    Fuel,
    Lumber,
    Blink
}

impl BountyEnum {
    pub fn min(self) -> i32 {
        match self {
            BountyEnum::Gold => 4,
            BountyEnum::Fuel => 6,
            BountyEnum::Lumber => 6,
            BountyEnum::Blink => 4
        }
    }

    pub fn amount(self, rng: &mut ChaCha20Rng) -> i32 {
        match self {
            BountyEnum::Gold => 50,
            BountyEnum::Fuel => MSG_FUEL * 3,
            BountyEnum::Lumber => 60,
            BountyEnum::Blink => 0
        }
    }

    pub fn color(self) -> Color {
        match self {
            BountyEnum::Blink => Color::BLUE,
            BountyEnum::Fuel => Color::BLACK,
            BountyEnum::Gold => Color::ORANGE,
            BountyEnum::Lumber => Color::BROWN,
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Bounty {
    pub type_: BountyEnum,
    pub amount: i32,
    #[serde(with = "Vector2Def")]
    pub pos: Vector2,
}

#[derive(Clone)]
pub struct GameState {
    pub my_units: Vec<Unit>,
    pub other_units: Vec<Unit>,
    // TODO this should actually be (bool, bool, HashSet<unit_id>)
    // currently it is possible to select mutliple ships/stations
    pub selection: HashSet<Selection>,
    pub sub_selection: Option<SubSelection>,
    pub fuel: [i32; 2],
    pub intercepted: [u8; 2],
    pub gold: [f32; 2],
    pub upgrades: [HashSet<Upgrade>; 2],
    pub items: [HashMap<Item, i16>; 2],
    pub spawn_cooldown: [i32; 2],
    pub bounties: Vec<Bounty>,
    pub last_bounty: HashMap<BountyEnum, i32>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(remote = "Vector2")]
struct Vector2Def {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Target {
    Move,
    Blink,
}

#[serde_nested]
#[derive(Clone, Serialize, Deserialize)]
pub struct Unit {
    pub dead: bool,
    pub player_id: usize,
    #[serde(with = "Vector2Def")]
    pub pos: Vector2,
    #[serde_nested(sub = "Vector2", serde(with = "Vector2Def"))]
    pub path: VecDeque<Vector2>,
    pub blinking: Option<bool>,
    pub blink_cooldown: i32,
    pub carrying_bounty: HashMap<BountyEnum, i32>,
}

pub fn unit_rect(pos: &Vector2, size: &Vector2) -> Rect<i32> {
    // TODO we want to round here the same way opengl does when drawing to the screen.
    Rect { 
        x: pos.x.round() as i32,
        y: pos.y.round() as i32, 
        w: size.x.round() as i32,
        h: size.y.round() as i32
    }
}

impl Unit {
    pub fn rect(self: &Self) -> Rect<i32> {
        unit_rect(&self.pos, MESSAGE_SIZE)
    }

    pub fn size(self: &Self) -> &Vector2 {
        MESSAGE_SIZE
    }

    pub fn speed(self: &Self) -> f32 {
        0.5f32
    }

    pub fn cooldown(self: &Self) -> i32 {
        BLINK_COOLDOWN
    }

    pub fn p0_colors(self: &Self) -> Color {
        rcolor(0x90, 0xE0, 0xEF, 255)
    }

    pub fn p1_colors(self: &Self) -> Color {
        rcolor(0x74, 0xC6, 0x9D, 255)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct BlinkCommand {
    pub u_id: usize,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct InterceptCommand {
    #[serde(with = "Vector2Def")]
    pub pos: Vector2
}

#[serde_nested]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnMsgCommand {
    pub player_id: usize,
    #[serde_nested(sub = "Vector2", serde(with = "Vector2Def"))]
    pub path: VecDeque<Vector2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameCommand {
    Blink(BlinkCommand),
    Spawn(SpawnMsgCommand),
    Intercept(InterceptCommand),
    BuyUpgrade(Upgrade),
    BuyItem(Item),
}

#[derive(Deserialize, Serialize)]
pub enum ClientPkt {
    Hello { seq: i32, sent_time: f64 },
    Target { seq: i32, ack: i32, updates: VecDeque<(i64, Vec<GameCommand>)>, frame: i64, frame_ack: i64 },
    Ended { seq: i32, ack: i32, frame: i64 },
    StateHash { seq: i32, ack: i32, hash: u32, frame: i64 }
}

#[derive(Deserialize, Serialize)]
pub struct ServerPkt {
    pub seq: i32,
    pub ack: i32,
    pub server_time: f64,
    pub msg: ServerEnum,
}

#[derive(Deserialize, Serialize)]
pub enum ServerEnum {
    Welcome { handshake_start_time: f64, player_id: usize },
    Start { rng_seed: [u8; 32] },
    UpdateOtherTarget { updates: VecDeque<(i64, Vec<GameCommand>)>, frame: i64, frame_ack: i64 }
}