use crate::{TimeWindowAvg, WindowAvg};

#[derive(Debug)]
pub enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started,
    Ended(Option<usize>),
}

pub struct NetInfo<'a> {
    pub game_ps: &'a TimeWindowAvg,
    pub waiting_avg: &'a WindowAvg,
    pub my_frame_delay: u8,
}