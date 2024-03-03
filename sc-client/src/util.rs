
use std::net::{SocketAddr, UdpSocket};
use sc_types::{ServerPkt, ClientPkt};
use std::io;

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::core::mem::size_of::<T>(),
    )
}

pub fn socket_recv(socket: &UdpSocket, expected_addr: &SocketAddr) -> Option<ServerPkt> {
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

pub fn socket_send(socket: &UdpSocket, addr: &SocketAddr, pkt: &ClientPkt) -> Result<usize, std::io::Error> {
    let to_send = unsafe { any_as_u8_slice(pkt) };
    socket.send_to(to_send, addr)
}