#[derive(Debug)]
pub enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started,
    Ended(Option<usize>),
}

/*
 * We update the GameState only after sending and receiving the inputs for the current frame
 * i.e. when frame_state = FrameState::Both.
 */
#[derive(Eq, PartialEq)]
pub enum FrameState {
    Neither,
    Sent,
    Received,
    Both,
}

impl FrameState {
    pub fn recvd(self: &mut Self) {
        match self {
            FrameState::Neither => *self = FrameState::Received,
            FrameState::Sent => *self = FrameState::Both,
            _ => {},
        }
    }

    pub fn sent(self: &mut Self) {
        match self {
            FrameState::Neither => *self = FrameState::Sent,
            FrameState::Received => *self = FrameState::Both,
            _ => {},
        }
    }
}