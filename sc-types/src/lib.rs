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

#[derive(Default)]
#[derive(Clone)]
pub struct GameState {
    pub pos: [Vector2; 2],
    pub target: [Vector2; 2],
}

pub enum ClientPkt {
    Hello { seq: i32, sent_time: f64 },
    Target { seq: i32, ack: i32, pos: Vector2, target: Vector2, frame: i64 },
}

pub struct ServerPkt {
    pub seq: i32,
    pub ack: i32,
    pub server_time: f64,
    pub msg: ServerEnum,
}

pub enum ServerEnum {
    Welcome { handshake_start_time: f64, player_id: usize },
    Start { state: GameState },
    UpdateOtherTarget { other_pos: Vector2, other_target: Vector2, frame: i64 }
}