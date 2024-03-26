// extern crate serde;
extern crate serde_derive;

use std::collections::{HashSet, VecDeque};
use constants::{BLINK_COOLDOWN, MESSAGE_SIZE};
use raylib::prelude::{Vector2,Color};

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

#[derive(Clone)]
pub struct GameState {
    pub my_units: Vec<Unit>,
    pub other_units: Vec<Unit>,
    pub selection: HashSet<Selection>,
    pub sub_selection: Option<SubSelection>,
    pub fuel: [i32; 2],
    pub intercepted: [u8; 2],
    pub gold: [f32; 2],
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
    pub blinking: bool,
    pub cooldown: i32,
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
        2f32
    }

    pub fn cooldown(self: &Self) -> i32 {
        BLINK_COOLDOWN
    }

    pub fn p0_colors(self: &Self) -> Color {
        Color::from_hex("90E0EF").unwrap()
    }

    pub fn p1_colors(self: &Self) -> Color {
        Color::from_hex("74C69D").unwrap()
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
    pub path: VecDeque<Vector2>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameCommand {
    Blink(BlinkCommand),
    Spawn(SpawnMsgCommand),
    Intercept(InterceptCommand),
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
    Start,
    UpdateOtherTarget { updates: VecDeque<(i64, Vec<GameCommand>)>, frame: i64, frame_ack: i64 }
}