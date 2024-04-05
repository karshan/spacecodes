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
    history: Vec<f64>,
    index: usize,
    size: usize,
    pub avg: f64,
    pub max: f64,
}

impl WindowAvg {
    pub fn new(size: usize) -> WindowAvg {
        let mut hist = vec![];
        for _i in 0..size {
            hist.push(0f64);
        }
        WindowAvg {
            history: hist,
            index: 0,
            size: size,
            avg: 0f64,
            max: 0f64
        }
    }

    pub fn sample(self: &mut Self, v: f64) -> f64 {
        self.index = (self.index + 1) % self.size;
        self.avg -= self.history[self.index];
        self.history[self.index] = v/(self.size as f64);
        self.avg += self.history[self.index];
        self.max = *self.history[0..self.size].iter().max_by(|x, y| x.total_cmp(*y)).unwrap();
        self.avg
    }

    pub fn one_percent_max(self: &Self) -> f64 {
        let mut tmp: Vec<f64> = self.history.iter().cloned().map(|v| v * self.size as f64).collect();
        tmp.sort_by(|x, y| y.partial_cmp(x).unwrap());
        let t2: Vec<f64> = tmp.iter().cloned().take(self.size/100).collect();
        if t2.is_empty() {
            -1f64
        } else {
            t2.iter().fold(0f64, |acc, e| acc + *e)/(t2.len() as f64)
        }
    }
}