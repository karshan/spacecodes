use raylib::prelude::Vector2;

#[derive(Default)]
pub struct SeqState {
    expected_seq: i32,
    expected_ack: i32,
    pub send_seq: i32,
    pub send_ack: i32,
}

impl SeqState {
    pub fn recv(&self, seq: i32, ack: i32) -> SeqState {
        if ack != self.expected_ack {
            panic!("Expected ack {} got {}", self.expected_ack, ack)
        }

        if seq != self.expected_seq {
            panic!("Expected seq {} got {}", self.expected_seq, seq)
        }

        SeqState {
            expected_seq: seq + 1,
            expected_ack: ack,
            send_seq: self.send_seq,
            send_ack: seq,
        }
    }

    pub fn send(&self) -> SeqState {
        SeqState {
            expected_seq: self.expected_seq,
            expected_ack: self.send_seq,
            send_seq: self.send_seq + 1,
            send_ack: self.send_ack,
        }
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
    Target { seq: i32, ack: i32, target: Vector2 },
}

pub enum ServerPkt {
    Welcome { seq: i32, ack: i32, handshake_start_time: f64, player_id: usize },
    Start { seq: i32, ack: i32, state: GameState },
    UpdateOtherTarget { seq: i32, ack: i32, other_target: Vector2, frame: i64 }
}