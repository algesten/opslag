use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub struct FormatIpv4Addr(pub Ipv4Addr);

impl defmt::Format for FormatIpv4Addr {
    fn format(&self, fmt: defmt::Formatter) {
        let octets = self.0.octets();
        defmt::write!(
            fmt,
            "{}.{}.{}.{}",
            octets[0],
            octets[1],
            octets[2],
            octets[3]
        );
    }
}

pub struct FormatIpv6Addr(pub Ipv6Addr);

impl defmt::Format for FormatIpv6Addr {
    fn format(&self, fmt: defmt::Formatter) {
        let segments = self.0.segments();
        defmt::write!(
            fmt,
            "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7]
        );
    }
}

pub struct FormatIpAddr(pub IpAddr);

impl defmt::Format for FormatIpAddr {
    fn format(&self, fmt: defmt::Formatter) {
        match self.0 {
            IpAddr::V4(addr) => defmt::write!(fmt, "{}", FormatIpv4Addr(addr)),
            IpAddr::V6(addr) => defmt::write!(fmt, "{}", FormatIpv6Addr(addr)),
        }
    }
}

pub struct FormatSocketAddr(pub SocketAddr);

impl defmt::Format for FormatSocketAddr {
    fn format(&self, fmt: defmt::Formatter) {
        match self.0 {
            SocketAddr::V4(addr) => {
                defmt::write!(fmt, "{}:{}", FormatIpv4Addr(*addr.ip()), addr.port());
            }
            SocketAddr::V6(addr) => {
                defmt::write!(fmt, "[{}]:{}", FormatIpv6Addr(*addr.ip()), addr.port());
            }
        }
    }
}
