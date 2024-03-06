use raylib::prelude::Vector2;

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
    pub my_units: [Option<(UnitEnum, Unit)>; 10],
    pub other_units: [Option<(UnitEnum, Unit)>; 10],
    pub selection: u8,
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
    Stop
}

#[derive(Debug, Copy, Clone)]
pub struct Unit {
    pub player_id: u8,
    pub pos: Vector2,
    pub dir: Dir,
}

#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum UnitEnum {
    MessageBox,
    Interceptor
}

#[derive(Debug, Copy, Clone)]
pub enum GameCommand {
    Move(u8, Dir),
    Spawn(u8, UnitEnum, Unit)
}

pub enum ClientPkt {
    Hello { seq: i32, sent_time: f64 },
    Target { seq: i32, ack: i32, updates: [GameCommand; 10], frame: i64 },
}

pub struct ServerPkt {
    pub seq: i32,
    pub ack: i32,
    pub server_time: f64,
    pub msg: ServerEnum,
}

pub enum ServerEnum {
    Welcome { handshake_start_time: f64, player_id: u8 },
    Start,
    UpdateOtherTarget { updates: [GameCommand; 10], frame: i64 }
}