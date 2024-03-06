use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use std::fmt;
use raylib::prelude::*;
use sc_types::*;

mod util;
use util::*;

enum ClientState {
    SendHello,
    ExpectWelcome,
    Waiting,
    Started(bool),
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClientState::SendHello => write!(f, "SendHello"),
            ClientState::ExpectWelcome => write!(f, "ExpectWelcome"),
            ClientState::Waiting => write!(f, "Waiting"),
            ClientState::Started(b) => write!(f, "Started {}", b),
        }
    }
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
    let frame_rate = 15;
    let player_speed = [ 5.0f32, 8f32 ];
    let player_size = Vector2 { x: 10.0, y: 10.0 };

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
    rl.set_target_fps(frame_rate);

    let mut state = ClientState::SendHello;
    let mut game_state: GameState = Default::default();
    let mut p_id = 0;
    let mut seq_state: SeqState = Default::default();
    let mut frame_counter: i64 = 0;
    let mut s_time = 0f64;
    let mut sent_frame = 0;
    let mut unsent_target = Vector2::zero();
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    while !rl.window_should_close() {
        let mut go = false;

        state = match state {
            ClientState::SendHello => {
                socket_send(&socket, &server[0], &ClientPkt::Hello { seq: seq_state.send_seq, sent_time: rl.get_time() })?;
                seq_state.send();
                ClientState::ExpectWelcome
            },
            ClientState::ExpectWelcome => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => ClientState::ExpectWelcome,
                    Some(ServerEnum::Welcome { handshake_start_time, player_id }) => {
                        p_id = player_id;
                        ClientState::Waiting
                    },
                    Some(_) => {
                        panic!("Expected Welcome")
                    },
                }
            },
            ClientState::Waiting => {
                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => ClientState::Waiting,
                    Some(ServerEnum::Start { state }) => {
                        game_state = state;
                        frame_counter = 0;
                        unsent_target = game_state.target[p_id];
                        ClientState::Started(false)
                    },
                    Some(_) => {
                        panic!("Expected Start")
                    }
                }
            },
            ClientState::Started(ended) => {
                let other_id = (p_id + 1) % 2;

                let resp = socket_recv(&socket, &server[0], &mut seq_state, &mut s_time);
                match resp {
                    None => {},
                    Some(ServerEnum::UpdateOtherTarget { other_pos, other_target, frame }) => {
                        game_state.target[other_id] = other_target;
                        go = true;
                    },
                    Some(_) => {
                        panic!("Expected UpdateOtherTarget")
                    }
                }

                if rl.is_mouse_button_released(MouseButton::MOUSE_LEFT_BUTTON) {
                    unsent_target = rl.get_mouse_position();
                }

                if sent_frame <= frame_counter {
                    socket_send(&socket, &server[0], &ClientPkt::Target { 
                        seq: seq_state.send_seq,
                        ack: seq_state.send_ack,
                        pos: game_state.pos[p_id],
                        target: unsent_target,
                        frame: frame_counter,
                    })?;
                    seq_state.send();

                    game_state.target[p_id] = unsent_target;
                    sent_frame += 1;
                }

                if go && !ended {
                    for i in 0..2 {
                        game_state.pos[i] = move_(game_state.pos[i], game_state.target[i], player_speed[i]);
                    }
                    frame_counter += 1;
                }

                if (game_state.pos[0].x - game_state.pos[1].x).abs() < player_size.x &&
                    (game_state.pos[0].y - game_state.pos[1].y).abs() < player_size.y {
                    ClientState::Started(true)
                } else {
                    ClientState::Started(false)
                }
            },
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);

        for i in 0..2 {
            d.draw_rectangle_v(game_state.pos[i], player_size, if i == 0 { Color::RED } else { Color::BLACK });
        }

        match state { 
            ClientState::Started(true) => {
                d.draw_text(&format!("{:?} {:?}", game_state.pos[0], game_state.pos[1]), 20, 20, 20, Color::BLACK);
                d.draw_text(&format!("{:?} {:?}", game_state.target[0], game_state.target[1]), 20, 40, 20, Color::BLACK);
                d.draw_text(&frame_counter.to_string(), 20, 60, 20, Color::BLACK);
            },
            _ => {}
        }
    }
    Ok(())
}