use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::env;
use std::io;
use std::fmt;
use raylib::prelude::*;
use sc_types::*;

enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started,
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClientState::SendHello => write!(f, "SendHello"),
            ClientState::ExpectWelcome => write!(f, "ExpectWelcome"),
            ClientState::Waiting => write!(f, "Waiting"),
            ClientState::Started => write!(f, "Started"),
        }
    }
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::core::mem::size_of::<T>(),
    )
}

fn socket_recv(socket: &UdpSocket, expected_addr: &SocketAddr) -> Option<ServerPkt> {
    let mut buf = [0u8; 48];
    match socket.recv_from(&mut buf) {
        Ok((n, addr)) => {
            if addr != *expected_addr {
                panic!("Expected server_addr: {} got {}", expected_addr, addr)
            }
            if n != 48 {
                panic!("Expected 48 bytes got {}", n)
            }

            let resp: ServerPkt = unsafe { std::mem::transmute::<[u8; 48], ServerPkt>(buf) };
            Some(resp)
        },
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            None
        }
        Err(e) => panic!("encountered IO error: {e}"),
    }
}

fn socket_send(socket: &UdpSocket, addr: &SocketAddr, pkt: &ClientPkt) -> Result<usize, std::io::Error> {
    let to_send = unsafe { any_as_u8_slice(pkt) };
    socket.send_to(to_send, addr)
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
    let mut game_state = Default::default();
    let mut p_id = 0;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        state = match state {
            ClientState::SendHello => {
                socket_send(&socket, &server[0], &ClientPkt::Hello { seq: 0, sent_time: rl.get_time() })?;
                ClientState::ExpectWelcome
            },
            ClientState::ExpectWelcome => {
                let resp = socket_recv(&socket, &server[0]);
                match resp {
                    None => ClientState::ExpectWelcome,
                    Some(ServerPkt::Welcome { seq, ack, handshake_start_time, player_id }) => {
                        latency = rl.get_time() - handshake_start_time;
                        p_id = player_id;
                        ClientState::Waiting
                    },
                    Some(_) => {
                        panic!("Expected Welcome")
                    },
                }
            },
            ClientState::Waiting => {
                let resp = socket_recv(&socket, &server[0]);
                match resp {
                    None => ClientState::Waiting,
                    Some(ServerPkt::Start { seq, ack, state }) => {
                        game_state = state;
                        ClientState::Started
                    },
                    Some(_) => {
                        panic!("Expected Start")
                    }
                }
            },
            ClientState::Started => {
                let mut o_tgt = None;
                let other_id = (p_id + 1) % 2;

                let resp = socket_recv(&socket, &server[0]);
                match resp {
                    None => {},
                    Some(ServerPkt::UpdateOtherTarget { seq, ack, other_target, frame }) => {
                        o_tgt = Some(other_target);
                    },
                    Some(_) => {
                        panic!("Expected UpdateOtherTarget")
                    }
                }

                if rl.is_mouse_button_released(MouseButton::MOUSE_LEFT_BUTTON) {
                    game_state.target[p_id] = rl.get_mouse_position();
                    socket_send(&socket, &server[0], &ClientPkt::Target { seq: 0, ack: 0, target: game_state.target[p_id] })?;
                }

                match o_tgt {
                    Some(t) => game_state.target[other_id] = t,
                    _ => {}
                };

                for i in 0..2 {
                    game_state.pos[i] = move_(game_state.pos[i], game_state.target[i], 1.0);
                }

                ClientState::Started
            }
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);

        let player_size = Vector2 { x: 10.0, y: 10.0 };
        for i in 0..2 {
            d.draw_rectangle_v(game_state.pos[i], player_size, if p_id == i { Color::RED } else { Color::BLACK });
        }

        d.draw_text(&state.to_string(), 20, 20, 20, Color::BLACK);
        d.draw_text(&((latency * 1000_f64).round() as i64).to_string(), 20, 40, 20, Color::BLACK);
    }
    Ok(())
}