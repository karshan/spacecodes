use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use std::io;
use std::fmt;
use raylib::prelude::*;
use sc_types::*;

enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting(usize),
    Started(usize, GameState)
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClientState::SendHello => write!(f, "SendHello"),
            ClientState::ExpectWelcome => write!(f, "ExpectWelcome"),
            ClientState::Waiting(_) => write!(f, "Waiting"),
            ClientState::Started(_, _) => write!(f, "Started"),
        }
    }
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::core::mem::size_of::<T>(),
    )
}

fn move_(pos: Vector2, target: Vector2, speed: f32) -> Vector2 {
    let delta = target - pos;
    if delta.length_sqr() < speed * speed { 
        target
    } else { 
        pos + delta.normalized().scale_by(speed)
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage {} server_addr", args[0]);
        std::process::exit(1);
    }
    let server_addr = &args[1][..];

    let server: Vec<std::net::SocketAddr> = server_addr
        .to_socket_addrs()
        .expect("Unable to resolve domain")
        .collect();
    if server.len() < 1 {
        panic!("unable to resolve server?")
    }

    let (mut rl, thread) = raylib::init()
        .size(640, 480)
        .title("Space Codes")
        .build();
    rl.set_target_fps(60);

    let mut state = ClientState::SendHello;
    let mut latency = 0_f64;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mut gs = None;

        state = match state {
            ClientState::SendHello => {
                let client_pkt = ClientPkt::Hello { seq: 0, sent_time: rl.get_time() };
                let to_send = unsafe { any_as_u8_slice(&client_pkt) };
                socket.send_to(to_send, server[0])?;
                ClientState::ExpectWelcome
            },
            ClientState::ExpectWelcome => {
                let mut buf = [0u8; 48];
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        // TODO ignore if addr != server_addr
                        if n != 48 {
                            panic!("Expected 48 bytes got {}", n)
                        }

                        let resp: ServerPkt = unsafe { std::mem::transmute::<[u8; 48], ServerPkt>(buf) };
                        match resp {
                            ServerPkt::Welcome { seq, ack, handshake_start_time, player_id } => {
                                latency = rl.get_time() - handshake_start_time;
                                ClientState::Waiting(player_id)
                            },
                            _ => {
                                panic!("Expected Welcome")
                            }
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        ClientState::ExpectWelcome
                    }
                    Err(e) => panic!("encountered IO error: {e}"),
                }
            },
            ClientState::Waiting(player_id) => {
                let mut buf = [0u8; 48];
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        // TODO ignore if addr != server_addr
                        if n != 48 {
                            panic!("Expected 48 bytes got {}", n)
                        }

                        let resp: ServerPkt = unsafe { std::mem::transmute::<[u8; 48], ServerPkt>(buf) };
                        match resp {
                            ServerPkt::Start { seq, ack, state } => {
                                ClientState::Started(player_id, state)
                            },
                            _ => {
                                panic!("Expected Start")
                            }
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        ClientState::Waiting(player_id)
                    }
                    Err(e) => panic!("encountered IO error: {e}"),
                }
            },
            ClientState::Started(player_id, GameState { pos, target }) => {
                let mut buf = [0u8; 48];
                let mut o_tgt = None;
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        // TODO ignore if addr != server_addr
                        if n != 48 {
                            panic!("Expected 48 bytes got {}", n)
                        }

                        let resp: ServerPkt = unsafe { std::mem::transmute::<[u8; 48], ServerPkt>(buf) };
                        match resp {
                            ServerPkt::UpdateOtherTarget { seq, ack, other_target, frame } => {
                                o_tgt = Some(other_target);
                            },
                            _ => {
                                panic!("Expected UpdateOtherTarget")
                            }
                        }
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    }
                    Err(e) => panic!("encountered IO error: {e}"),
                }

                let my_target = if rl.is_mouse_button_released(MouseButton::MOUSE_LEFT_BUTTON) {
                    let tmp = rl.get_mouse_position();
                    let client_pkt = ClientPkt::Target { seq: 0, ack: 0, target: tmp };
                    let to_send = unsafe { any_as_u8_slice(&client_pkt) };
                    socket.send_to(to_send, server[0])?;
                    tmp
                } else {
                    target[player_id]
                };

                let mut new_target = [target[0], target[1]];
                new_target[player_id] = my_target;
                match o_tgt {
                    Some(t) => new_target[(player_id + 1) % 2] = t,
                    _ => {}
                };
                let new_pos = [move_(pos[0], new_target[0], 1.0), move_(pos[1], new_target[1], 1.0)];

                let next_gs = GameState { pos: new_pos, target: new_target };
                gs = Some((player_id, next_gs.clone()));
                ClientState::Started(player_id, next_gs)
            }
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        match gs {
            Some((player_id, GameState { pos, target })) => {
                d.draw_rectangle_v(pos[0], Vector2 { x: 10.0, y: 10.0 }, if player_id == 0 { Color::RED } else { Color::BLACK });
                d.draw_rectangle_v(pos[1], Vector2 { x: 10.0, y: 10.0 }, if player_id == 1 { Color::RED } else { Color::BLACK });
            },
            None => {}
        }

        d.draw_text(&state.to_string(), 20, 20, 20, Color::BLACK);
    }
    Ok(())
}