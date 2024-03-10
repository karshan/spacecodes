// extern crate serde;
#[macro_use]
extern crate serde_derive;

use std::collections::HashSet;
use raylib::prelude::{Vector2,Color};

pub mod shapes;
use shapes::*;

#[derive(Default)]
pub struct SeqState {
    expected_seq: i32,
    expected_ack: i32,
    pub send_seq: i32,
    pub send_ack: i32,
}

impl SeqState {
    pub fn recv(&mut self, seq: i32, ack: i32) {
        if ack > self.expected_ack {
            panic!("Expected ack {} got {}", self.expected_ack, ack)
        }

        if seq != self.expected_seq {
            panic!("Expected seq {} got {}", self.expected_seq, seq)
        }

        self.expected_seq = seq + 1;
        self.send_ack = seq;
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

#[derive(Clone)]
pub struct GameState {
    // FIXME switch to one collection for my+other units so order of operations is the same on all clients
    pub my_units: Vec<Unit>,
    pub other_units: Vec<Unit>,
    pub selection: HashSet<Selection>,
    pub fuel: [i32; 2],
    pub intercepted: [u8; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vector2")]
struct Vector2Def {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub type_: UnitEnum,
    pub player_id: usize,
    #[serde(with = "Vector2Def")]
    pub pos: Vector2,
    #[serde(with = "Vector2Def")]
    pub target: Vector2,
    pub cooldown: i32,
}

impl Unit {
    pub fn rect(self: &Self) -> Rect<i32> {
         // TODO we want to round here the same way opengl does when drawing to the screen.
        Rect { 
            x: self.pos.x.round() as i32,
            y: self.pos.y.round() as i32, 
            w: self.type_.size().x.round() as i32,
            h: self.type_.size().y.round() as i32
        }
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum UnitEnum {
    MessageBox,
    Interceptor(i32),
    Dead,
}

impl UnitEnum {
    pub fn size(self: &Self) -> &'static Vector2 {
        match self {
            UnitEnum::Interceptor(_) => &Vector2 { x: 20f32, y: 20f32 },
            UnitEnum::MessageBox => &Vector2 { x: 20f32, y: 20f32 },
            UnitEnum::Dead => &Vector2 { x: 0f32, y: 0f32 },
        }
    }

    pub fn speed(self: &Self) -> f32 {
        match self {
            UnitEnum::Interceptor(_) => 1f32,
            UnitEnum::MessageBox => 1f32,
            UnitEnum::Dead => 0f32,
        }
    }

    pub fn cooldown(self: &Self) -> i32 {
        match self {
            UnitEnum::Interceptor(_) => 360,
            UnitEnum::MessageBox => 0,
            UnitEnum::Dead => 0,
        }
    }

    pub fn p0_colors(self: &Self) -> Color {
        match self {
            UnitEnum::Interceptor(_) => Color::from_hex("90E0EF").unwrap(),
            UnitEnum::MessageBox => Color::from_hex("90E0EF").unwrap(),
            UnitEnum::Dead => Color::BLACK,
        }
    }

    pub fn p1_colors(self: &Self) -> Color {
        match self {
            UnitEnum::Interceptor(_) => Color::from_hex("74C69D").unwrap(),
            UnitEnum::MessageBox => Color::from_hex("74C69D").unwrap(),
            UnitEnum::Dead => Color::BLACK,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct MoveCommand {
    pub u_id: usize,
    #[serde(with = "Vector2Def")]
    pub target: Vector2,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct InterceptCommand {
    pub u_id: usize,
    #[serde(with = "Vector2Def")]
    pub pos: Vector2,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum GameCommand {
    Move(MoveCommand),
    Spawn(Unit),
    Intercept(InterceptCommand),
}

#[derive(Deserialize, Serialize)]
pub enum ClientPkt {
    Hello { seq: i32, sent_time: f64 },
    Target { seq: i32, ack: i32, updates: Vec<GameCommand>, frame: i64 },
    Ended { seq: i32, ack: i32, frame: i64 },
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
    UpdateOtherTarget { updates: Vec<GameCommand>, frame: i64 }
}