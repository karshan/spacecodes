extern crate rmp_serde as rmps;

use std::{collections::{HashMap, VecDeque}, hash::Hash, net::{SocketAddr, UdpSocket}, ops::AddAssign, slice::Iter, time::Instant};
use num_traits::Zero;
use raylib::{color::{rcolor, Color}, math::{Vector2, Vector3}};
use sc_types::{ClientPkt, SeqState, ServerEnum, ServerPkt};
use std::io;

pub fn scale_color(a: Color, s: f32) -> Color {
    let b = |x: u8| (x as f32 * s).round().min(255.0) as u8;
    rcolor(b(a.r), b(a.g), b(a.b), a.a)
}

pub fn vec3(v2: Vector2, z: f32) -> Vector3 {
    Vector3::new(v2.x, v2.y, z)
}

pub fn vec2(v3: Vector3) -> Vector2 {
    Vector2::new(v3.x, v3.y)
}

pub fn rounded(v: Vector2) -> Vector2 {
    Vector2::new(v.x.round(), v.y.round())
}

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

pub struct FrameMap<T>(Vec<(i64, T)>);

impl<T: Clone + PartialEq> FrameMap<T> {
    pub fn new() -> FrameMap<T> {
        FrameMap(vec![])
    }

    pub fn iter<'a>(self: &'a Self) -> Iter<'a, (i64, T)> {
        self.0.iter()
    }

    pub fn retain<F>(self: &mut Self, f: F)
    where
        F: FnMut(&(i64, T)) -> bool,
    {
        self.0.retain(f)
    }

    pub fn push(self: &mut Self, k: i64, v: T) {
        if self.0.iter().any(|(f, _)| *f == k) {
            panic!("trying to overwrite frame in FrameMap");
        }
        self.0.push((k, v));
    }

    pub fn merge(self: &mut Self, other: &VecDeque<(i64, T)>) {
        for (k, v) in other {
            match self.0.iter().find(|(f, _)| *f == *k) {
                Some((_, existing_v)) => {
                    if existing_v != v {
                        panic!("trying to overwrite frame in FrameMap with different value");
                    }
                },
                None => {
                    self.push(*k, v.clone());
                }
            }
        }
    }

    pub fn cloned_vecdeque(self: &Self) -> VecDeque<(i64, T)> {
        VecDeque::from(self.0.clone())
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