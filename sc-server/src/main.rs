use async_std::io;
use async_std::net::UdpSocket;
use async_std::task;
use sc_types::*;
use std::collections::HashMap;
use raylib::prelude::Vector2;

enum ServerState {
    Waiting,
    Started(GameState)
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::core::mem::size_of::<T>(),
    )
}

fn main() -> io::Result<()> {
    task::block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:8080").await?;
        let mut buf = [0u8; 24];
        let mut conn_states = HashMap::new();
        let mut state = ServerState::Waiting;

        println!("Listening on {}", socket.local_addr()?);

        loop {
            let (n, peer) = socket.recv_from(&mut buf).await?;
            let req: ClientPkt = unsafe { std::mem::transmute::<[u8; 24], ClientPkt>(buf) };
            conn_states.entry(peer).or_default();

            match req {
                ClientPkt::Hello { seq, sent_time } => {
                    let p_id = conn_states.len() - 1;
                    let seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    *seq_state = seq_state.recv(seq, 0);
                    let server_pkt = ServerPkt::Welcome {
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        handshake_start_time: sent_time,
                        player_id: p_id
                    };
                    let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                    socket.send_to(to_send, peer).await?;
                    *seq_state = seq_state.send();
                },
                ClientPkt::Target { seq, ack, target } => {
                    let r_seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    *r_seq_state = r_seq_state.recv(seq, ack);
                    match state {
                        ServerState::Started(_) => {
                            for (send_peer, s_seq_state) in conn_states.iter_mut() {
                                if *send_peer != peer {
                                    let server_pkt = ServerPkt::UpdateOtherTarget { seq: s_seq_state.send_seq, ack: s_seq_state.send_ack, other_target: target, frame: 0 };
                                    let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                                    socket.send_to(to_send, send_peer).await?;
                                    *s_seq_state = s_seq_state.send();
                                }
                            }
                        },
                        ServerState::Waiting => {}
                    }
                }
            }

            match state {
                ServerState::Waiting => {
                    if conn_states.len() >= 2 {
                        let gs = GameState {
                            pos: [Vector2 { x: 0.0, y: 0.0 }, Vector2 { x: 100.0, y: 0.0 }],
                            target: [Vector2 { x: 0., y: 0.0 }, Vector2 { x: 100.0, y: 0.0 }],
                        };
                        for (peer, seq_state) in conn_states.iter_mut() {
                            let server_pkt = ServerPkt::Start { seq: seq_state.send_seq, ack: seq_state.send_ack, state: gs.clone() };
                            let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                            socket.send_to(to_send, peer).await?;
                            *seq_state = seq_state.send();
                        }
                        state = ServerState::Started(gs)
                    }
                }
                _ => {}
            }
        }
    })
}