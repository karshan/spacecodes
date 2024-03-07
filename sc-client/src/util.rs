extern crate rmp_serde as rmps;

use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};

use std::net::{SocketAddr, UdpSocket};
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