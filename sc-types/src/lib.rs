use raylib::prelude::Vector2;

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