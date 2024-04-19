use std::{net::UdpSocket, time::Instant};

use sc_types::{ClientPkt, GameCommand, SeqState, ServerEnum};

use crate::{socket_recv, socket_send, ClientState, FrameMap, WindowAvg};

pub fn handle_handshake(state: ClientState, socket: &UdpSocket, server: &Vec<std::net::SocketAddr>, seq_state: &mut SeqState, p_id: &mut usize)
    // startGame with this seed
    -> (Option<[u8; 32]>, ClientState) {
    match state {
        ClientState::SendHello => {
            socket_send(&socket, &server[0], &ClientPkt::Hello { seq: seq_state.send_seq, sent_time: 0.0 }).unwrap();
            seq_state.send();
            (None, ClientState::ExpectWelcome)
        },
        ClientState::ExpectWelcome => {
            let resp = socket_recv(&socket, &server[0], seq_state);
            match resp {
                None => (None, ClientState::ExpectWelcome),
                Some(ServerEnum::Welcome { handshake_start_time: _, player_id }) => {
                    *p_id = player_id;
                    (None, ClientState::Waiting)
                },
                Some(_) => {
                    panic!("Expected Welcome")
                },
            }
        },
        ClientState::Waiting => {
            let resp = socket_recv(&socket, &server[0], seq_state);
            match resp {
                None => (None, ClientState::Waiting),
                Some(ServerEnum::Start { rng_seed }) => {
                    (Some(rng_seed), ClientState::Started)
                },
                Some(_) => {
                    panic!("Expected Start")
                }
            }
        },
        _ => (None, state)
    }
}

pub static MAX_PKT_QUEUE: usize = 40;
pub struct NetState {
    pub next_send_frame: i32,
    pub unsent_pkt: Vec<GameCommand>,
    pub unacked_pkts: FrameMap<Vec<GameCommand>>,
    pub future_pkts: FrameMap<Vec<GameCommand>>, //rename recvd_pkts
    pub sent_pkts: FrameMap<Vec<GameCommand>>,
    pub last_rcvd_pkt: i32,
    pub my_frame_delay: u8,
    pub m_new_frame_delay: Option<u8>,
    pub waiting: Instant,
    pub waiting_avg: WindowAvg,
}

impl NetState {
    pub fn new() -> NetState {
        let default_fram_delay = 1;
        let mut future_pkts = FrameMap::new();
        let mut sent_pkts = FrameMap::new();
        for i in 0..default_fram_delay {
            future_pkts.push(i as i32, vec![]);
            sent_pkts.push(i as i32, vec![]);                            
        }
        NetState {
            next_send_frame: 0,
            unsent_pkt: vec![],
            unacked_pkts: FrameMap::new(),
            future_pkts,
            sent_pkts,
            last_rcvd_pkt: -1,
            my_frame_delay: default_fram_delay,
            m_new_frame_delay: None,
            waiting: Instant::now(),
            waiting_avg: WindowAvg::new(600),
        }
    }

    pub fn queue_command(self: &mut Self, command: GameCommand) {
        if self.unsent_pkt.len() < MAX_PKT_QUEUE {
            self.unsent_pkt.push(command);
        }
    }

    pub fn process(self: &mut Self, frame_counter: i32, socket: &UdpSocket, server: &Vec<std::net::SocketAddr>, seq_state: &mut SeqState, frame_rate: u32) 
        -> Option<(Vec<GameCommand>, Vec<GameCommand>)> {
        let resp = socket_recv(&socket, &server[0], seq_state);
        match resp {
            None => {}
            Some(ServerEnum::UpdateOtherTarget { updates, frame, frame_ack, frame_delay: _ }) => {
                self.waiting_avg.sample(self.waiting.elapsed().as_secs_f64());
                self.waiting = Instant::now();
                self.future_pkts.merge(&updates.clone());
                self.unacked_pkts.retain(|ps| ps.0 > frame_ack);
                self.last_rcvd_pkt = frame;
            },
            Some(_) => {
                panic!("Expected UpdateOtherTarget")
            }
        }

        if self.next_send_frame <= frame_counter {
            let mut dont_send = false;
            if let Some(new_frame_delay) = self.m_new_frame_delay {
                if new_frame_delay > self.my_frame_delay {
                    for i in self.my_frame_delay..new_frame_delay {
                        self.unacked_pkts.push(frame_counter + i as i32, vec![]);
                        self.sent_pkts.push(frame_counter + i as i32, vec![]);
                    }
                    self.m_new_frame_delay = None;
                    self.my_frame_delay = new_frame_delay;
                } else {
                    if self.sent_pkts.iter().any(|(f, _)| *f >= frame_counter + new_frame_delay as i32) {
                        dont_send = true;
                    } else {
                        self.m_new_frame_delay = None;
                        self.my_frame_delay = new_frame_delay;
                    }
                }
            }
            if !dont_send {
                self.unacked_pkts.push(frame_counter + self.my_frame_delay as i32, self.unsent_pkt.clone());
                socket_send(&socket, &server[0], &ClientPkt::Target { 
                    seq: seq_state.send_seq,
                    ack: seq_state.send_ack,
                    updates: self.unacked_pkts.cloned_vecdeque(),
                    frame: frame_counter + self.my_frame_delay as i32,
                    frame_ack: self.last_rcvd_pkt,
                    frame_delay: self.my_frame_delay
                }).unwrap();
                seq_state.send();
                self.sent_pkts.push(frame_counter + self.my_frame_delay as i32, self.unsent_pkt.clone());
                self.unsent_pkt = vec![];
            }
            self.next_send_frame += 1;
        }

        // next_send_frame > frame_counter should be equivalent to sent_pkts.any(.0 == frame_counter)
        let result = if (self.next_send_frame > frame_counter) && self.future_pkts.iter().any(|ps| ps.0 == frame_counter) {
            let recvd_pkt = self.future_pkts.iter().find(|ps| ps.0 == frame_counter).unwrap().1.clone();
            let sent_pkt = self.sent_pkts.iter().find(|ps| ps.0 == frame_counter).unwrap().1.clone();
            self.future_pkts.retain(|ps| ps.0 > frame_counter);
            self.sent_pkts.retain(|ps| ps.0 > frame_counter);
            Some((sent_pkt, recvd_pkt))
        } else {
            None
        };

        let waiting_one_pct_max = f64::min(self.waiting_avg.one_percent_max(), 300f64/1000f64);
        if self.m_new_frame_delay.is_none() {
            let new_delay = (waiting_one_pct_max * (frame_rate as f64)).ceil() as i32;
            let mfd = self.my_frame_delay as i32;
            if new_delay > mfd || new_delay < mfd/2 {
                self.m_new_frame_delay = Some(new_delay as u8);
            }
        }
        result
    }
}