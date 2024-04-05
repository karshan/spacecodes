extern crate rmp_serde as rmps;

use std::{collections::HashMap, hash::Hash, net::{SocketAddr, UdpSocket}, ops::AddAssign, time::Instant};
use num_traits::Zero;
use sc_types::{ClientPkt, SeqState, ServerEnum, ServerPkt};
use std::io;

pub fn hm_add<K: Hash + Clone + Copy + Eq, V: AddAssign + Copy + Clone>(a: HashMap<K, V>, b: &HashMap<K, V>) -> HashMap<K, V> {
    let mut out = a.clone();
    for (k, v) in b.iter() {
        out.entry(*k).and_modify(|e| *e += *v).or_insert(*v);
    }
    out
}

pub fn socket_recv(socket: &UdpSocket, expected_addr: &SocketAddr, seq_state: &mut SeqState) -> Option<ServerEnum> {
    let mut buf = [0u8; 16000];
    match socket.recv_from(&mut buf) {
        Ok((n, addr)) => {
            if addr != *expected_addr {
                panic!("Expected server_addr: {} got {}", expected_addr, addr)
            }

            match rmp_serde::decode::from_slice::<ServerPkt>(&buf[..n]) {
                Ok(pkt) => {
                    seq_state.recv(pkt.seq, pkt.ack);
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
    match rmp_serde::encode::to_vec(pkt) {
        Ok(buf) => socket.send_to(&buf, addr),
        Err(e) => panic!("{:?}", e),
    }
}

pub struct TimeWindowAvg {
    history: [f64; 30],
    last: Instant,
    index: usize,
    pub avg: f64,
}

impl TimeWindowAvg {
    pub fn new() -> TimeWindowAvg {
        TimeWindowAvg {
            history:[0f64; 30],
            last: Instant::now(),
            index: 0,
            avg: 0f64,
        }
    }

    pub fn get_hz(self: &Self) -> f64 {
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
        self.index = (self.index + 1) % 30;
        self.avg -= self.history[self.index];
        self.history[self.index] = (dt.as_millis() as f64)/(30 as f64);
        self.avg += self.history[self.index];
        self.get_hz()
    }
}

pub struct WindowAvg {
    history: [f64; 30],
    index: usize,
    pub avg: f64,
}

impl WindowAvg {
    pub fn new() -> WindowAvg {
        WindowAvg {
            history:[0f64; 30],
            index: 0,
            avg: 0f64,
        }
    }

    pub fn sample(self: &mut Self, v: f64) -> f64 {
        self.index = (self.index + 1) % 30;
        self.avg -= self.history[self.index];
        self.history[self.index] = v/(30 as f64);
        self.avg += self.history[self.index];
        self.avg
    }
}