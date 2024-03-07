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
pub struct GameState {
    pub my_units: Vec<(UnitEnum, Unit)>,
    pub other_units: Vec<(UnitEnum, Unit)>,
    pub selection: usize,
    pub fuel: [i32; 2],
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
    Stop
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
    pub dir: Dir,
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum UnitEnum {
    MessageBox,
    Interceptor
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum GameCommand {
    Move(usize, Dir),
    Spawn(UnitEnum, Unit)
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