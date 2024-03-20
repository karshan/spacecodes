extern crate rmp_serde as rmps;

use std::{net::{SocketAddr, UdpSocket}, time::Instant};
use num_traits::Zero;
use sc_types::{ClientPkt, SeqState, ServerEnum, ServerPkt};
use std::io;

pub fn socket_recv(socket: &UdpSocket, expected_addr: &SocketAddr, seq_state: &mut SeqState, s_time: &mut f64) -> Option<ServerEnum> {
    let mut buf = [0u8; 1024];
    match socket.recv_from(&mut buf) {
        Ok((n, addr)) => {
            if addr != *expected_addr {
                panic!("Expected server_addr: {} got {}", expected_addr, addr)
            }

            match rmp_serde::decode::from_slice::<ServerPkt>(&buf[..n]) {
                Ok(pkt) => {
                    seq_state.recv(pkt.seq, pkt.ack);
                    *s_time = pkt.server_time;
                    Some(pkt.msg)
                },
                Err(e) => panic!("{:?}", e)
            }
        },
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            None
        }
        Err(e) => panic!("encountered IO error: {e}"),
    }
}

pub fn socket_send(socket: &UdpSocket, addr: &SocketAddr, pkt: &ClientPkt) -> Result<usize, std::io::Error> {
    match  rmp_serde::encode::to_vec(pkt) {
        Ok(buf) => socket.send_to(&buf, addr),
        Err(e) => panic!("{:?}", e),
    }
}

static WINDOW_SIZE: usize = 30;
pub struct WindowAvg {
    history: [f64; 30],
    last: Instant,
    index: usize,
    pub avg: f64,
}

impl WindowAvg {
    pub fn new() -> WindowAvg {
        WindowAvg {
            history:[0f64; 30],
            last: Instant::now(),
            index: 0,
            avg: 0f64,
        }
    }

    pub fn peek(self: &Self) -> f64 {
        if self.avg.is_zero() {
            0f64
        } else {
            1000f64/self.avg
        }
    }

    pub fn sample(self: &mut Self) -> f64 {
        let now = Instant::now();
        let dt = now.duration_since(self.last);
        self.last = now;
        self.index = (self.index + 1) % WINDOW_SIZE;
        self.avg -= self.history[self.index];
        self.history[self.index] = (dt.as_millis() as f64)/(WINDOW_SIZE as f64);
        self.avg += self.history[self.index];
        self.peek()
    }
}