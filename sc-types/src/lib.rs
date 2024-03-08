// extern crate serde;
#[macro_use]
extern crate serde_derive;

use raylib::prelude::Vector2;
// use serde::{Deserialize, Serialize};

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

#[derive(Clone)]
pub enum Selection {
    Unit(usize),
    Station,
    Ship,
}

#[derive(Clone)]
pub struct GameState {
    pub my_units: Vec<(UnitEnum, Unit)>,  // FIXME switch to HashMap for units so deletion doesn't mess up selection
    pub other_units: Vec<(UnitEnum, Unit)>,
    pub selection: Selection,
    pub fuel: [i32; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Vector2")]
struct Vector2Def {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub player_id: usize,
    #[serde(with = "Vector2Def")]
    pub pos: Vector2,
    #[serde(with = "Vector2Def")]
    pub target: Vector2,
    pub cooldown: i32,
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum UnitEnum {
    MessageBox,
    Interceptor,
    Dead,
}

impl UnitEnum {
    pub fn size(self: &Self) -> &'static Vector2 {
        match self {
            UnitEnum::Interceptor => &Vector2 { x: 20f32, y: 20f32 },
            UnitEnum::MessageBox => &Vector2 { x: 20f32, y: 20f32 },
            UnitEnum::Dead => &Vector2 { x: 0f32, y: 0f32 },
        }
    }

    pub fn speed(self: &Self) -> f32 {
        match self {
            UnitEnum::Interceptor => 1f32,
            UnitEnum::MessageBox => 1f32,
            UnitEnum::Dead => 0f32,
        }
    }

    pub fn cooldown(self: &Self) -> i32 {
        match self {
            UnitEnum::Interceptor => 360,
            UnitEnum::MessageBox => 0,
            UnitEnum::Dead => 0,
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
    Spawn(UnitEnum, Unit),
    Intercept(InterceptCommand),
}

#[derive(Deserialize, Serialize)]
pub enum ClientPkt {
    Hello { seq: i32, sent_time: f64 },
    Target { seq: i32, ack: i32, updates: Vec<GameCommand>, frame: i64 },
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