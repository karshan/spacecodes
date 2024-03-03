use async_std::io;
use async_std::net::UdpSocket;
use async_std::task;
use sc_types::{ClientPkt, GameState, ServerPkt};
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

            match req {
                ClientPkt::Hello { seq, sent_time } => {
                    let server_pkt = ServerPkt::Welcome { seq: 0, ack: seq, handshake_start_time: sent_time, player_id: conn_states.len() };
                    let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                    socket.send_to(to_send, peer).await?;
                },
                ClientPkt::Target { seq, ack, target } => {
                    match state {
                        ServerState::Started(_) => {
                            for (send_peer, _) in &conn_states {
                                if *send_peer != peer {
                                    let server_pkt = ServerPkt::UpdateOtherTarget { seq: 0, ack: 0, other_target: target, frame: 0 };
                                    let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                                    socket.send_to(to_send, send_peer).await?;
                                }
                            }
                        },
                        ServerState::Waiting => {

                        }
                    }
                }
            }

            if !conn_states.contains_key(&peer) {
                conn_states.insert(peer, 0);
            }

            match state {
                ServerState::Waiting => {
                    if conn_states.len() >= 2 {
                        let gs = GameState {
                            pos: [Vector2 { x: 0.0, y: 0.0 }, Vector2 { x: 100.0, y: 0.0 }],
                            target: [Vector2 { x: 0., y: 0.0 }, Vector2 { x: 100.0, y: 0.0 }],
                        };
                        for (peer, _) in &conn_states {
                            let server_pkt = ServerPkt::Start { seq: 0, ack: 0, state: gs.clone() };
                            let to_send = unsafe { any_as_u8_slice(&server_pkt) };
                            socket.send_to(to_send, peer).await?;
                        }
                        state = ServerState::Started(gs)
                    }
                }
                _ => {}
            }
        }
    })
}