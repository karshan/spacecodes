#[derive(Debug)]
pub enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started,
    Ended(Option<usize>),
}