use std::net::{ToSocketAddrs, UdpSocket};
use std::env;
use net::{handle_handshake, NetState};
use rand_core::SeedableRng;
use raylib::prelude::*;
use sc_types::*;
extern crate rmp_serde as rmps;
use rand_chacha::*;

mod util;
mod types;
mod render;
mod net;
mod game;

use game::*;
use util::*;
use types::*;

use crate::render::Renderer;

fn main() -> std::io::Result<()> {
    let frame_rate = 60;

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
        .title("Space Codes")
        .msaa_4x()
        .build();
    rl.set_trace_log(TraceLogLevel::LOG_ERROR);
    rl.set_window_icon(Image::load_image_from_mem(".png", include_bytes!("../assets/icon.png")).unwrap());
    rl.set_target_fps(frame_rate);

    set_non_fullscreen_window_size(&mut rl);

    let mut render = Renderer::new(&mut rl, &thread);

    let mut state = ClientState::SendHello;
    // Most of these values doesn't matter. Its just for the compiler. They are initialized in ClientState::Waiting
    let mut game_state: GameState = GameState::new(0, ChaCha20Rng::from_seed([0; 32]));
    let mut seq_state = SeqState::new();
    let mut frame_counter: i32 = 0;
    let mut net = NetState::new();
    let mut mouse_state: MouseState = MouseState::None;
    let mut game_ps = TimeWindowAvg::new();

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;

    rl.set_exit_key(None);

    let mut zoom = false;
    let mut borderless = false;
    while !rl.window_should_close() {
        let raw_mouse_position = rl.get_mouse_position();
        let screen_width =  rl.get_screen_width() as f64;
        let screen_height = rl.get_screen_height() as f64;
        let mouse_position = Renderer::screen2world(raw_mouse_position, screen_width, screen_height, zoom);
        let mut screen_changed = false;

        let (m_start_with_seed, new_state) = handle_handshake(state, &socket, &server, &mut seq_state, &mut game_state.p_id);
        state = new_state;
        if let Some(rng_seed) = m_start_with_seed {
            frame_counter = 0;
            net = NetState::new();
            mouse_state = MouseState::None;
            game_state = GameState::new(game_state.p_id, ChaCha20Rng::from_seed(rng_seed));
        }
    
        state = match state {
            ClientState::Started => {
                run_game(&mut game_state, &mut screen_changed, &mut zoom, &mut borderless,
                    &mut rl, &mut mouse_state, &mut net, &mut frame_counter, &socket, &server, &mut seq_state, frame_rate, &mut game_ps)
            },
            ClientState::Ended(end_state) => {
                if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
                    seq_state = SeqState::new();
                    ClientState::SendHello
                } else {
                    ClientState::Ended(end_state)
                }
            },
            _ => state
        };

        render.render(&mut rl, &thread, frame_counter, &game_state, mouse_position, &mouse_state, &state, zoom,
            &NetInfo { game_ps: &game_ps, waiting_avg: &net.waiting_avg, my_frame_delay: net.my_frame_delay }, screen_changed);
    }
    socket_send(&socket, &server[0], &ClientPkt::Disconnect).unwrap();
    Ok(())
}