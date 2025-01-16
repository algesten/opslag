use core::net::{IpAddr, Ipv4Addr};

use crate::dns::{self, Answer, Label, QClass, QType, Record};
use crate::format::FormatIpAddr;
use crate::vec::Vec;

/// Information about a service to declare over mDNS.
#[derive(Debug)]
pub struct ServiceInfo<'a, const LLEN: usize = 4> {
    service_type: Label<'a, LLEN>,
    instance_name: Label<'a, LLEN>,
    host_name: Label<'a, LLEN>,
    ip_address: IpAddr,
    port: u16,
}

const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

impl<'a, const LLEN: usize> ServiceInfo<'a, LLEN> {
    /// Creates information about a new service.
    ///
    /// ```
    /// use opslag::ServiceInfo;
    ///
    /// let info = ServiceInfo::<4>::new(
    ///    "_my-service._udp.local", // Name of my service, same for all nodes
    ///    "instance01",             // This specific service instance
    ///    "nugget.local",           // My host name (<some_name>.local)
    ///    [192, 168, 0, 3],         // The IP for my host name
    ///    1234,                     // The port the service is running on
    /// );
    /// ```
    pub fn new(
        service_type: &'a str,
        instance_name: &'a str,
        host_name: &'a str,
        ip_address: impl Into<IpAddr>,
        port: u16,
    ) -> Self {
        let service_type = Label::new(service_type);

        let mut i = service_type.clone();
        i.push_front(instance_name);

        Self {
            service_type,
            instance_name: i,
            host_name: Label::new(host_name),
            ip_address: ip_address.into(),
            port,
        }
    }

    /// The type of service.
    ///
    /// Example: `_my-service._tcp.local`
    pub fn service_type(&self) -> &Label<'a, LLEN> {
        &self.service_type
    }

    /// The instance name.
    ///
    /// Example: `myinstance01`
    pub fn instance_name(&self) -> &Label<'a, LLEN> {
        &self.instance_name
    }

    /// The host name the service is running on.
    ///
    /// Example: `Martin's Macbook Air.local`
    pub fn host_name(&self) -> &Label<'a, LLEN> {
        &self.host_name
    }

    /// Corresponding IP address for the host name.
    ///
    /// Example: `192.160.10.24`
    pub fn ip_address(&self) -> IpAddr {
        self.ip_address
    }

    /// Port the service is running on.
    ///
    /// Example: `8080`
    pub fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn ptr_answer(&'a self, _aclass: QClass) -> Answer<'a, LLEN> {
        Answer {
            name: self.service_type.clone(),
            atype: QType::PTR,
            aclass: QClass::IN,
            ttl: 4500,
            record: Record::PTR(dns::PTR {
                name: self.instance_name.clone(),
            }),
        }
    }

    pub(crate) fn srv_answer(&'a self, aclass: QClass) -> Answer<'a, LLEN> {
        Answer {
            name: self.instance_name.clone(),
            atype: QType::SRV,
            aclass,
            ttl: 120,
            record: Record::SRV(dns::SRV {
                priority: 0,
                weight: 0,
                port: self.port,
                target: self.host_name.clone(),
            }),
        }
    }

    pub(crate) fn txt_answer(&'a self, aclass: QClass) -> Answer<'a, LLEN> {
        Answer {
            name: self.instance_name.clone(),
            atype: QType::TXT,
            aclass,
            ttl: 120,
            record: Record::TXT(dns::TXT { text: "\0" }),
        }
    }

    pub(crate) fn ip_answer(&'a self, aclass: QClass) -> Answer<'a, LLEN> {
        match self.ip_address {
            IpAddr::V4(address) => Answer {
                name: self.host_name.clone(),
                atype: QType::A,
                aclass,
                ttl: 120,
                record: Record::A(dns::A { address }),
            },
            IpAddr::V6(address) => Answer {
                name: self.host_name.clone(),
                atype: QType::AAAA,
                aclass: QClass::IN,
                ttl: 120,
                record: Record::AAAA(dns::AAAA { address }),
            },
        }
    }

    pub(crate) fn from_answers<const SLEN: usize>(
        answers: &[Answer<'a, LLEN>],
        output: &mut Vec<Self, SLEN>,
    ) {
        // Step 1: Process PTR records
        for answer in answers {
            if let Record::PTR(ptr) = &answer.record {
                let instance_name = ptr.name.clone();
                let service_type = answer.name.clone();
                let _ = output.push(ServiceInfo {
                    service_type,
                    instance_name,
                    host_name: Label::default(),
                    ip_address: DEFAULT_ADDR,
                    port: 0,
                });
            }
        }

        // Step 2: Process SRV records and merge data
        for answer in answers {
            if let Record::SRV(srv) = &answer.record {
                for stub in output.iter_mut() {
                    if stub.instance_name == answer.name {
                        stub.host_name = srv.target.clone();
                        stub.port = srv.port;
                    }
                }
            }
        }

        // Step 3: Process A and AAAA records and merge data
        for answer in answers {
            match &answer.record {
                Record::A(a) => {
                    for stub in output.iter_mut() {
                        if stub.host_name == answer.name {
                            stub.ip_address = IpAddr::V4(a.address);
                        }
                    }
                }
                Record::AAAA(aaaa) => {
                    for stub in output.iter_mut() {
                        if stub.host_name == answer.name {
                            stub.ip_address = IpAddr::V6(aaaa.address);
                        }
                    }
                }
                _ => {}
            }
        }

        // Final step: Retain only complete services
        output.retain(|stub| {
            !stub.service_type.is_empty()
                && !stub.instance_name.is_empty()
                && !stub.host_name.is_empty()
                && stub.ip_address != DEFAULT_ADDR
                && stub.port != 0
        });
    }

    pub(crate) fn as_answers(
        &'a self,
        aclass: QClass,
    ) -> impl Iterator<Item = Answer<'a, LLEN>> + 'a {
        [
            self.ptr_answer(aclass),
            self.srv_answer(aclass),
            self.txt_answer(aclass),
            self.ip_answer(aclass),
        ]
        .into_iter()
    }
}

impl<'a, const LLEN: usize> defmt::Format for ServiceInfo<'a, LLEN> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "ServiceInfo {{ service_type: {}, instance_name: {}, host_name: {}, ip_address: {}, port: {} }}",
            self.service_type,
            self.instance_name,
            self.host_name,
            FormatIpAddr(self.ip_address),
            self.port
        );
    }
}
