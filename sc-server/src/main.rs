use async_std::io;
use async_std::net::UdpSocket;
use async_std::task;
use sc_types::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

enum ServerState {
    Waiting,
    Started,
    Ended(SocketAddr),
}

fn main() -> io::Result<()> {
    task::block_on(async {
        let socket = UdpSocket::bind("0.0.0.0:8080").await?;
        let mut buf = [0u8; 1024];
        let mut conn_states = HashMap::new();
        let mut state_hashes = HashMap::new();
        let mut state = ServerState::Waiting;
        let mut instant = Instant::now();

        println!("Listening on {}", socket.local_addr()?);

        loop {
            let (n, peer) = socket.recv_from(&mut buf).await?;

            let req = match rmp_serde::decode::from_slice::<ClientPkt>(&buf[..n]) {
                Ok(pkt) => {
                    pkt
                },
                Err(e) => panic!("{:?}", e)
            };
            conn_states.entry(peer).or_default();

            match req {
                ClientPkt::Hello { seq, sent_time } => {
                    let p_id = conn_states.len() - 1;
                    let seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    seq_state.recv(seq, 0);
                    let server_pkt = ServerPkt {
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        server_time: instant.elapsed().as_secs_f64(),
                        msg: ServerEnum::Welcome {
                            handshake_start_time: sent_time,
                            player_id: p_id
                        }
                    };
                    match  rmp_serde::encode::to_vec(&server_pkt) {
                        Ok(buf) => {
                            socket.send_to(&buf, peer).await?;
                            seq_state.send();
                        }
                        Err(e) => panic!("{:?}", e),
                    }
                },
                ClientPkt::Target { seq, ack, updates, frame } => {
                    let r_seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    r_seq_state.recv(seq, ack);
                    match state {
                        ServerState::Started => {
                            for (send_peer, s_seq_state) in conn_states.iter_mut() {
                                if *send_peer != peer {
                                    let server_pkt = ServerPkt {
                                        seq: s_seq_state.send_seq,
                                        ack: s_seq_state.send_ack,
                                        server_time: instant.elapsed().as_secs_f64(),
                                        msg: ServerEnum::UpdateOtherTarget { updates: updates.clone(), frame: frame },
                                    };
                                    match  rmp_serde::encode::to_vec(&server_pkt) {
                                        Ok(buf) => {
                                            socket.send_to(&buf, send_peer).await?;
                                            s_seq_state.send();
                                        }
                                        Err(e) => panic!("{:?}", e),
                                    }
                                }
                            }
                        },
                        ServerState::Waiting => {},
                        ServerState::Ended(_) => {},
                    }
                },
                ClientPkt::Ended { seq, ack, frame } => {
                    let r_seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    r_seq_state.recv(seq, ack);
                    match state {
                        ServerState::Started => {
                            state = ServerState::Ended(peer)
                        },
                        ServerState::Ended(ended_addr) => {
                            if peer != ended_addr {
                                conn_states.clear();
                                state = ServerState::Waiting
                            }
                        },
                        _ => {}
                    }

                },
                ClientPkt::StateHash { seq, ack, hash, frame } => {
                    let r_seq_state: &mut SeqState = conn_states.get_mut(&peer).expect("Peer not in hashmap");
                    r_seq_state.recv(seq, ack);
                    if *state_hashes.entry(frame).or_insert(hash) != hash {
                        println!("Mismatched hashes on frame {}", frame)
                    }
                    if frame >= 10 {
                        state_hashes.remove(&(frame - 10));
                    }
                }
            }

            match state {
                ServerState::Waiting => {
                    if conn_states.len() >= 2 {
                        instant = Instant::now();
                        for (peer, seq_state) in conn_states.iter_mut() {
                            let server_pkt = ServerPkt {
                                seq: seq_state.send_seq,
                                ack: seq_state.send_ack,
                                server_time: instant.elapsed().as_secs_f64(),
                                msg: ServerEnum::Start,
                            };
                            match  rmp_serde::encode::to_vec(&server_pkt) {
                                Ok(buf) => {
                                    socket.send_to(&buf, peer).await?;
                                    seq_state.send();
                                }
                                Err(e) => panic!("{:?}", e),
                            }
                        }
                        state = ServerState::Started
                    }
                }
                _ => {}
            }
        }
    })
}