use core::net::{IpAddr, SocketAddr};

use crate::dns::{Flags, Message, QClass, QType, Query, Request, Response};
use crate::time::Time;
use crate::vec::Vec;
use crate::writer::Writer;
use crate::ServiceInfo;

/// A server for broadcasting/discovering peers.
///
/// * `QLEN` - Max number of queries in a single mDNS packet. Only used if not **std**.
///            Typically 4 for SRV, PTR, TXT and A (or AAAA).
/// * `ALEN` - Max number of answers in a single mDNS packet. Only used if not **std**.
///            Typically 4 for SRV, PTR, TXT and A (or AAAA).
/// * `LLEN` - Max number of segments for a parsed Label.
///            All services have max 4 segments: martin_test._myservice._udp.local.
/// * `SLEN` - Number of service infos to handle in the [`Server`].
/// * `LK`   – List size for DNS label compression. 10 is a good value.
///
/// Specifying too small QLEN, ALEN, LLEN or SLEN does not make the server fail, but rather
/// reject messages that can't be parsed.
///
/// ```
/// use opslag::{Server, ServiceInfo};
///
/// let info = ServiceInfo::<4>::new(
///     "_midiriff._udp.local", // name of service
///     "martin_test",          // instance name, in case multiple services on same host
///     "mini.local",           // host
///     [192, 168, 0, 1],       // IP address of host
///     [255, 255, 255, 0],     // Netmask for the IP
///     1234,                   // port of service
///  );
///
/// // Max 4 queries
/// // Max 4 answers
/// // Max 4 segments in a label.
/// // 1 handled service
/// // 10 entries for dns label compression
/// let server = Server::<4, 4, 4, 1, 10>::new([info].into_iter());
/// ```
pub struct Server<
    'a,
    const QLEN: usize,
    const ALEN: usize,
    const LLEN: usize,
    const SLEN: usize,
    const LK: usize,
> {
    last_now: Time,
    services: Vec<ServiceInfo<'a, LLEN>, SLEN>,
    local_ips: Vec<LocalIp, SLEN>,
    next_advertise: Time,
    next_advertise_idx: usize,
    next_query: Time,
    next_query_idx: usize,
    txid_query: u16,
    next_txid: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LocalIp {
    addr: IpAddr,
    mask: IpAddr,
}

const ADVERTISE_INTERVAL: u64 = 15_000;
const QUERY_INTERVAL: u64 = 19_000;

/// How to cast outgoing packets.
#[derive(Debug)]
pub enum Cast {
    /// Send as multicast.
    Multi {
        /// Send from this ip address.
        from: IpAddr,
    },
    /// Unicast to specific socket address.
    Uni {
        /// Send from this ip address.
        from: IpAddr,
        /// Send to this ip address.
        target: SocketAddr,
    },
}

/// Input to [`Server`].
#[derive(Debug)]
pub enum Input<'x> {
    /// A timeout.
    ///
    /// It's fine to send timeouts when there is nothing else to do.
    /// The service expects a timeout for the [`Output::Timeout`] indicated.
    Timeout(Time),

    /// Some data coming from the network.
    Packet(&'x [u8], SocketAddr),
}

/// Output from the [`Server`].
pub enum Output<'x, const LLEN: usize, const SLEN: usize> {
    /// A packet to send somewhere.
    ///
    /// The data is in the buffer given to [`Server::handle`] and the amount of the
    /// buffer use is the first argument of the tuple.å
    Packet(usize, Cast),

    /// Next time the service expects a timeout.
    ///
    /// It is fine to send more timeouts before this.
    Timeout(Time),

    /// The [`Server`] discovered a remote instance of a declared [`ServiceInfo`].
    Remote(ServiceInfo<'x, LLEN>),
}

impl<
        'a,
        const QLEN: usize,
        const ALEN: usize,
        const LLEN: usize,
        const SLEN: usize,
        const LK: usize,
    > Server<'a, QLEN, ALEN, LLEN, SLEN, LK>
{
    /// Creates a new server instance.
    pub fn new(
        iter: impl Iterator<Item = ServiceInfo<'a, LLEN>>,
    ) -> Server<'a, QLEN, ALEN, LLEN, SLEN, LK> {
        let mut services = Vec::new();
        services.extend(iter);

        let mut local_ips = Vec::new();
        for s in services.iter() {
            let loc = LocalIp {
                addr: s.ip_address(),
                mask: s.netmask(),
            };
            let has_ip = local_ips.iter().any(|l| *l == loc);
            if !has_ip {
                // unwrap: this should be fine since local_ips is as long as services.
                local_ips.push(loc).unwrap();
            }
        }

        Server {
            last_now: Time::from_millis(0),
            services,
            local_ips,
            next_advertise: Time::from_millis(3000),
            next_advertise_idx: 0,
            next_query: Time::from_millis(5000),
            next_query_idx: 0,
            txid_query: 0,
            next_txid: 1,
        }
    }

    fn poll_timeout(&self) -> Time {
        self.next_advertise.min(self.next_query)
    }

    /// Handle some input and produce output.
    ///
    /// You can send [`Input::Timeout`] whenenver. The `buffer` is for outgoing packets.
    /// Upon [`Output::Packet`] the buffer will be filled to some point with data to transmit.
    pub fn handle<'x>(&mut self, input: Input<'x>, buffer: &mut [u8]) -> Output<'x, LLEN, SLEN> {
        match input {
            Input::Timeout(now) => self.handle_timeout(now, buffer),
            Input::Packet(data, from) => self.handle_packet(data, from, buffer),
        }
    }

    fn handle_timeout(&mut self, now: Time, buffer: &mut [u8]) -> Output<'static, LLEN, SLEN> {
        self.last_now = now;

        if now >= self.next_advertise {
            let send_from = self.local_ips[self.next_advertise_idx];

            let ret = self.do_advertise(buffer, send_from);

            self.next_advertise_idx += 1;

            if self.next_advertise_idx == self.local_ips.len() {
                self.next_advertise_idx = 0;
                self.next_advertise = now + ADVERTISE_INTERVAL;
            }

            ret
        } else if now >= self.next_query {
            let send_from = self.local_ips[self.next_query_idx];

            let ret = self.do_query(buffer, send_from);

            self.next_query_idx += 1;

            if self.next_query_idx == self.local_ips.len() {
                self.next_query_idx = 0;
                self.next_query = now + QUERY_INTERVAL;
            }

            ret
        } else {
            Output::Timeout(self.poll_timeout())
        }
    }

    fn next_txid(&mut self) -> u16 {
        let x = self.next_txid;
        self.next_txid = self.next_txid.wrapping_add(1);
        x
    }

    fn do_advertise(&mut self, buffer: &mut [u8], local: LocalIp) -> Output<'static, LLEN, SLEN> {
        let mut response: Response<QLEN, ALEN, LLEN> = Response {
            id: 0,
            flags: Flags::standard_response(),
            queries: Vec::new(),
            answers: Vec::new(),
        };

        let to_consider = self
            .services
            .iter()
            .filter(|s| s.ip_address() == local.addr && s.netmask() == local.mask);

        for service in to_consider {
            response
                .answers
                .extend(service.as_answers(QClass::Multicast));
        }

        debug!("Advertise response (from {}): {:?}", local.addr, response);

        let mut buf = Writer::<LK>::new(buffer);

        response.serialize(&mut buf);

        Output::Packet(buf.len(), Cast::Multi { from: local.addr })
    }

    fn do_query(&mut self, buffer: &mut [u8], local: LocalIp) -> Output<'static, LLEN, SLEN> {
        let mut request: Request<QLEN, LLEN> = Request {
            id: self.next_txid(),
            flags: Flags::standard_request(),
            queries: Vec::new(),
        };

        self.txid_query = request.id;

        let to_consider = self
            .services
            .iter()
            .filter(|s| s.ip_address() == local.addr && s.netmask() == local.mask);

        for service in to_consider {
            let query = Query {
                name: service.service_type().clone(),
                qtype: QType::PTR,
                qclass: QClass::IN,
            };
            request.queries.push(query).unwrap();
        }

        debug!("Send request (from {}): {:?}", local.addr, request);

        let mut buf = Writer::<LK>::new(buffer);
        request.serialize(&mut buf);

        Output::Packet(buf.len(), Cast::Multi { from: local.addr })
    }

    fn handle_packet<'x>(
        &mut self,
        data: &'x [u8],
        from: SocketAddr,
        buffer: &mut [u8],
    ) -> Output<'x, LLEN, SLEN> {
        match Message::parse(data) {
            Ok((_, Message::Request(request))) => self.handle_request(request, from, buffer),
            Ok((_, Message::Response(response))) => self.handle_response(response, from, buffer),
            Err(_) => Output::Timeout(self.poll_timeout()),
        }
    }

    fn handle_request<'x>(
        &mut self,
        request: Request<'x, QLEN, LLEN>,
        from: SocketAddr,
        buffer: &mut [u8],
    ) -> Output<'x, LLEN, SLEN> {
        if request.queries.is_empty() {
            return Output::Timeout(self.poll_timeout());
        }

        // Ignore requests from self
        if request.id == self.txid_query {
            return Output::Timeout(self.poll_timeout());
        }

        // We check for empty above
        let qclass = request.queries[0].qclass;

        let queries = request.queries.iter();

        let mut answers = Vec::new();

        for query in queries {
            for service in self.services.iter() {
                if query.qtype == QType::PTR
                    && &query.name == service.service_type()
                    && is_same_network(service.ip_address(), service.netmask(), from.ip())
                {
                    answers.extend(service.as_answers(qclass));
                }
            }
        }

        if answers.is_empty() {
            return Output::Timeout(self.poll_timeout());
        }

        debug!("Incoming request: {:?} {:?}", from, request);

        let response: Response<QLEN, ALEN, LLEN> = Response {
            id: request.id,
            flags: Flags::standard_response(),
            queries: request.queries,
            answers,
        };

        debug!("Send response: {:?}", response);
        let mut buf = Writer::<LK>::new(buffer);
        response.serialize(&mut buf);

        let send_from = self
            .local_ips
            .iter()
            .find(|l| is_same_network(l.addr, l.mask, from.ip()))
            // unwrap: is ok because above answers.is_empty() check means we must have had
            // a match between incoming query and service records.
            .unwrap()
            .addr;

        let cast = match qclass {
            QClass::IN => Cast::Uni {
                from: send_from,
                target: from,
            },
            _ => Cast::Multi { from: send_from },
        };

        Output::Packet(buf.len(), cast)
    }

    fn handle_response<'x>(
        &mut self,
        response: Response<'x, QLEN, ALEN, LLEN>,
        _from: SocketAddr,
        _buffer: &mut [u8],
    ) -> Output<'x, LLEN, SLEN> {
        let mut services = Vec::new();

        trace!("Handle response: {:?} {:?}", _from, response);

        ServiceInfo::from_answers::<SLEN>(&response.answers, &mut services);

        services.retain(|s| is_matching_service(s, &self.services));

        if services.len() > 1 {
            warn!("More than one service in answers. This is not currently handled");
        }

        if services.is_empty() {
            Output::Timeout(self.poll_timeout())
        } else {
            Output::Remote(services.remove(0))
        }
    }
}

fn is_same_network(ip: IpAddr, netmask: IpAddr, other: IpAddr) -> bool {
    match (ip, netmask, other) {
        (IpAddr::V4(ip), IpAddr::V4(mask), IpAddr::V4(other)) => {
            (u32::from(ip) & u32::from(mask)) == (u32::from(other) & u32::from(mask))
        }
        (IpAddr::V6(ip), IpAddr::V6(mask), IpAddr::V6(other)) => ip
            .segments()
            .iter()
            .zip(mask.segments().iter())
            .zip(other.segments().iter())
            .all(|((&ip_seg, &mask_seg), &other_seg)| {
                (ip_seg & mask_seg) == (other_seg & mask_seg)
            }),
        _ => false,
    }
}

fn is_matching_service<const LLEN: usize, const SLEN: usize>(
    s1: &ServiceInfo<'_, LLEN>,
    services: &Vec<ServiceInfo<'_, LLEN>, SLEN>,
) -> bool {
    let mut handled_service = false;
    let mut is_self = false;

    for s2 in services.iter() {
        handled_service |= s1.service_type() == s2.service_type();

        is_self |= s1.instance_name() == s2.instance_name()
            && s1.ip_address() == s2.ip_address()
            && s1.port() == s2.port();
    }

    handled_service && !is_self
}

#[cfg(feature = "defmt")]
impl defmt::Format for Input<'_> {
    fn format(&self, fmt: defmt::Formatter) {
        use crate::format::FormatSocketAddr;
        match self {
            Input::Timeout(instant) => {
                defmt::write!(fmt, "Timeout({:?})", instant);
            }
            Input::Packet(data, addr) => {
                defmt::write!(
                    fmt,
                    "Packet([..{} bytes], {:?})",
                    data.len(),
                    FormatSocketAddr(*addr)
                );
            }
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Cast {
    fn format(&self, fmt: defmt::Formatter) {
        use crate::format::{FormatIpAddr, FormatSocketAddr};
        match self {
            Cast::Multi { from } => {
                defmt::write!(fmt, "Multi {{ from:{:?} }}", FormatIpAddr(*from));
            }
            Cast::Uni { from, target } => {
                defmt::write!(
                    fmt,
                    "Uni {{ from:{:?}, target:{:?} }}",
                    FormatIpAddr(*from),
                    FormatSocketAddr(*target)
                );
            }
        }
    }
}
