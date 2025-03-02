# opslag

opslag is an mDNS library that is Sans-IO and no_std.

The library has generic types for parsing/serializing mDNS messages,
but the main functionality is [`Server`].

[`Server`] has two functions:

1. It advertises a local host/port under a service name.
2. It discovers other instances of the same service name.

The idea is that [`Server`] is used to create multiple nodes on the same
network that discover each other.

## Setup

```rust
use opslag::{Server, ServiceInfo};

// Declaration of what I want to advertise via mDNS.
// Expecting at most 4 segments to a DNS label.
let info = ServiceInfo::<4>::new(
    "_my-service._udp.local", // Name of my service, same for all nodes
    "martin_test",            // This specific service instance
    "nugget.local",           // My host name (<some_name>.local)
    [192, 168, 0, 3],         // The IP for my host name
    1234,                     // The port the service is running on
);

// The mDNS server.
// - max 4 queries per request
// - max 4 ansers per response
// - max 4 segments in a DNS label
// - 1 single service to announce
// - max 10 entries for DNS label compression
let mut server: Server<4, 4, 4, 1, 10> = Server::new([info]);
```

## Sans-IO and time

opslag is Sans-IO. That means sending and receiving data is an external
concern from the library. The [`Server`] can receive incoming data via
[`Input::Packet`] and instruct the user of the library to send something
via [`Output::Packet`].

The same goes for time. opslag has nothing driving time forwards internally.
It has timers that will trigger the periodic broadcast the handled services,
but driving time forwards is done via [`Input::Timeout`].

### Milliseconds

Conceptually when the [`Server`] is created, it is at time 0. Any [`Time`]
in [`Input::Timeout`], moves the internal clock forward. Each [`Time`] is
a millisecond offset from that time 0.

If we are using `std`, this is an example of how to create a `now()`
function that will give us an increasing time from a 0-point.

```rust
use std::time::Instant;
use opslag::{Time, Input};

// Instant at time 0
let start_time = Instant::now();

// Millisecond distance to time 0
let now = || {
  let ms = Instant::now()
    .saturating_duration_since(start_time)
    .as_millis() as u64;
  Time::from_millis(ms)
};

let input = Input::Timeout(now());
```

## The Loop

Below follows an example of how to construct a loop that handles
the IO and time. See `examples/myservice.rs` for a full working
example.

```rust
use opslag::{Time, Input, Output, Server, Cast, GROUP_SOCK_V4};
use std::time::Duration;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

// See above how to declare the server.
let server: Server<4,4,4,1,10> = todo!();

// Opening the UdpSocket is out of scope for this doc.
// See examples/myservice.rs for an example of how to do this.
let sock: UdpSocket = todo!();

// See above for a possible now() function.
let now: &dyn Fn() -> Time = &|| todo!();

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
```

License: MIT OR Apache-2.0
