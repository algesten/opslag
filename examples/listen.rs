use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use opslag::dns;
use socket2::{Domain, Type};

const MDNS_PORT: u16 = 5353;
const GROUP_ADDR_V4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);

pub fn main() {
    env_logger::init();

    // CHANGE THIS TO YOUR OWN IP:
    let my_ip: Ipv4Addr = "10.1.1.7".parse().unwrap();

    let addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), MDNS_PORT);
    let sock = socket2::Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

    sock.set_reuse_address(true).unwrap();

    #[cfg(unix)] // This is currently restricted to Unix's in socket2
    sock.set_reuse_port(true).unwrap();

    sock.bind(&addr.into()).unwrap();

    sock.join_multicast_v4(&GROUP_ADDR_V4, &my_ip).unwrap();

    sock.set_multicast_if_v4(&my_ip).unwrap();

    let mut buf = vec![0; 10 * 1024];

    let sock: UdpSocket = sock.into();

    loop {
        let (n, from) = sock.recv_from(&mut buf).unwrap();
        let buf = &buf[..n];
        println!("{:?}\n{:?}", from, buf);

        let (_, msg) = match dns::Message::<32, 32, 8>::parse(buf) {
            Ok(v) => v,
            Err(e) => {
                println!("ERROR: {:?}", e);
                continue;
            }
        };

        println!("{:?}", msg);
    }
}
