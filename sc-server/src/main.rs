use async_std::io;
use async_std::net::UdpSocket;
use async_std::task;
use std::collections::HashMap;

enum ConnState {
    Disconnected,
    Connected,
}

fn main() -> io::Result<()> {
    task::block_on(async {
        let socket = UdpSocket::bind("127.0.0.1:8080").await?;
        let mut buf = vec![0u8; 1024];
        let mut conn_states = HashMap::new();

        println!("Listening on {}", socket.local_addr()?);

        loop {
            let (n, peer) = socket.recv_from(&mut buf).await?;

            if conn_states.len() >= 2 {
                match conn_states.get(&peer) {
                    None => {
                        socket.send_to(b"FULL", &peer).await?;
                    },
                    Some(_conn_state) => {
                        println!("recvd {:?} from {}", String::from_utf8_lossy(&buf[..n]), peer);
                    },
                }
            } else {
                match conn_states.get(&peer) {
                    None => {
                        conn_states.insert(peer, ConnState::Connected);
                        socket.send_to(b"WLCM", &peer).await?;
                    },
                    Some(_conn_state) => {
                        socket.send_to(b"WAIT", &peer).await?;
                    },
                }
                
            }
        }
    })
}