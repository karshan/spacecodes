use raylib::prelude::Vector2;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::SeqCst;

#[derive(Default)]
pub struct SeqState {
    expected_seq: AtomicI32,
    expected_ack: AtomicI32,
    pub send_seq: AtomicI32,
    pub send_ack: AtomicI32,
}

impl SeqState {
    pub fn recv(&mut self, seq: i32, ack: i32) {
        let expected_ack = self.expected_ack.load(SeqCst);
        let expected_seq = self.expected_seq.load(SeqCst);
        if ack != expected_ack {
            panic!("Expected ack {} got {}", expected_ack, ack)
        }

        if seq != expected_seq {
            panic!("Expected seq {} got {}", expected_seq, seq)
        }

        self.expected_seq.fetch_add(1, SeqCst);
        self.send_ack.store(seq, SeqCst);
    }

    pub fn send(&mut self) {
        let send_seq = self.send_seq.load(SeqCst);
        self.expected_ack.store(send_seq, SeqCst);
        self.send_seq.fetch_add(1, SeqCst);
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