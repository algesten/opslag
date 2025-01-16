use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use opslag::{Cast, Input, Output, Server, ServiceInfo, Time};
use socket2::{Domain, Type};

const MDNS_PORT: u16 = 5353;
const GROUP_ADDR_V4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const GROUP_SOCK_V4: SocketAddrV4 = SocketAddrV4::new(GROUP_ADDR_V4, MDNS_PORT);

pub fn main() {
    env_logger::init();

    // CHANGE THIS TO YOUR OWN IP and host:
    let my_ip: Ipv4Addr = "10.0.0.54".parse().unwrap();
    let my_host = "nugget.local";

    // We must use socket2, because of set_reuse_port()
    let sock = socket2::Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

    // This makes it possible to listen to the 5353 port, even though
    // your system's main mDNS service (such as mDNSResponder on macOS)
    // also listens to it.
    #[cfg(unix)] // This is currently restricted to Unix's in socket2
    sock.set_reuse_port(true).unwrap();
    sock.set_reuse_address(true).unwrap();

    // Now we can bind the mDNS multicast address/port
    sock.bind(&GROUP_SOCK_V4.into()).unwrap();

    // Enable multicast
    sock.join_multicast_v4(&GROUP_ADDR_V4, &my_ip).unwrap();
    sock.set_multicast_if_v4(&my_ip).unwrap();

    // Convert socket2 -> regular std::net::UdpSocket
    let sock: UdpSocket = sock.into();

    // Declaration of what I want to advertise via mDNS.
    // Expecting at most 8 segments to a DNS label.
    let info = ServiceInfo::<4>::new(
        "_my-service._udp.local", // Name of my service, same for all nodes
        "martin_test",            // This specific service instance
        my_host,                  // My host name (<some_name>.local)
        my_ip,                    // The IP for my host name
        1234,                     // The port the service is running on
    );

    // The mDNS server.
    // We expect at most: 4 queries (QLEN), 4 answers (ALEN),
    // and 4 segments to DNS label (must match ServiceInfo).
    let mut server: Server<4, 4, 4, 1, 10> = Server::new([info]);

    // The server starts at some imaginary time 0. The `Time`
    // type encapsulates a number of milliseconds since this time
    // 0. Here we lock a start_time Instant for that time 0. And
    // the now() function gives us the milliseconds offset from
    // that start_time.
    let start_time = Instant::now();
    let now = || {
        let ms = Instant::now()
            .saturating_duration_since(start_time)
            .as_millis() as u64;
        Time::from_millis(ms)
    };

    // Buffers for receiving packets and writing output into.
    let mut packet = vec![0; 1024];
    let mut output = vec![0; 2048];

    // This will be set to the next timeout the server expects below.
    let mut next_timeout = now();

    // Next input to the server.
    let mut input = Input::Timeout(next_timeout);

    loop {
        match server.handle(input, &mut output) {
            Output::Packet(n, cast) => {
                // Send a packet to the give destination.
                let to_send = &output[..n];

                let target = match cast {
                    Cast::Multi => SocketAddr::V4(GROUP_SOCK_V4),
                    Cast::Uni(v) => v,
                };

                sock.send_to(to_send, target).unwrap();
            }
            Output::Timeout(time) => {
                // Next time the server expects a handle(Input::Timeout).
                next_timeout = time;
            }
            Output::Remote(service) => {
                // A discovered remote service.
                println!("Remote: {:#?}", service);
            }
        }

        // Check how long until the next timeout.
        let millis = now().millis_until(next_timeout);
        if millis == 0 {
            // Time is due right now (or already passed).
            input = Input::Timeout(now());
            continue;
        }

        // Timeout is in the future, make the socket wait that long.
        let dur = Duration::from_millis(millis);
        sock.set_read_timeout(Some(dur)).unwrap();

        let (n, from) = match sock.recv_from(&mut packet) {
            // New incoming packet
            Ok(v) => v,
            // Timeout reached
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                input = Input::Timeout(now());
                continue;
            }
            // Some other read error
            Err(e) => {
                eprintln!("Error reading from socket: {:?}", e);
                return;
            }
        };

        // Cue up this packet for Input::Packet when we loop
        let buf = &packet[..n];
        input = Input::Packet(buf, from);
    }
}
