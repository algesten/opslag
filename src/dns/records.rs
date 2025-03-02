use core::net::{Ipv4Addr, Ipv6Addr};
use core::str;
use nom::error::make_error;
use nom::{bytes::complete::take, number::complete::be_u16, IResult};

use super::query::QType;
use super::Label;
use crate::writer::Writer;

#[derive(Debug, PartialEq, Eq)]
// Enum for DNS-SD records
pub enum Record<'a, const LLEN: usize> {
    A(A),
    AAAA(AAAA),
    PTR(PTR<'a, LLEN>),
    TXT(TXT<'a>),
    SRV(SRV<'a, LLEN>),
}

impl<'a, const LLEN: usize> Record<'a, LLEN> {
    pub(crate) fn parse(
        input: &'a [u8],
        context: &'a [u8],
        record_type: QType,
    ) -> IResult<&'a [u8], Self> {
        trace!("Record::parse");
        match record_type {
            QType::A => {
                let (input, record) = A::parse(input)?;
                Ok((input, Record::A(record)))
            }
            QType::AAAA => {
                let (input, record) = AAAA::parse(input)?;
                Ok((input, Record::AAAA(record)))
            }
            QType::PTR => {
                let (input, record) = PTR::parse(input, context)?;
                Ok((input, Record::PTR(record)))
            }
            QType::TXT => {
                let (input, record) = TXT::parse(input)?;
                Ok((input, Record::TXT(record)))
            }
            QType::SRV => {
                let (input, record) = SRV::parse(input, context)?;
                Ok((input, Record::SRV(record)))
            }
            QType::Any => {
                warn!("Record::parse with ANY value");
                Err(nom::Err::Error(make_error(
                    input,
                    nom::error::ErrorKind::Tag,
                )))
            }
            QType::Unknown(_) => Err(nom::Err::Error(make_error(
                input,
                nom::error::ErrorKind::Tag,
            ))),
        }
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        match self {
            Record::A(record) => record.serialize(w),
            Record::AAAA(record) => record.serialize(w),
            Record::PTR(record) => record.serialize(w),
            Record::TXT(record) => record.serialize(w),
            Record::SRV(record) => record.serialize(w),
        }
    }
}

// Struct for A record
#[derive(Debug, PartialEq, Eq)]
pub struct A {
    pub address: Ipv4Addr,
}

impl A {
    pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], A> {
        trace!("A::parse");
        let (input, len) = be_u16(input)?;
        let (input, address) = take(len)(input)?;
        let address = Ipv4Addr::from(
            <[u8; 4]>::try_from(address)
                .map_err(|_| nom::Err::Failure(make_error(input, nom::error::ErrorKind::Fail)))?,
        );
        Ok((input, A { address }))
    }

    pub(crate) fn serialize<const LK: usize>(&self, w: &mut Writer<'_, '_, LK>) {
        let len = 4u16.to_be_bytes();
        w[..2].copy_from_slice(&len);
        w.inc(2);
        w[..4].copy_from_slice(&self.address.octets());
        w.inc(4);
    }
}

// Struct for AAAA record
#[derive(Debug, PartialEq, Eq)]
pub struct AAAA {
    pub address: Ipv6Addr,
}

impl AAAA {
    pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], AAAA> {
        trace!("AAAA::parse");
        let (input, len) = be_u16(input)?;
        let (input, address) = take(len)(input)?;
        let address = Ipv6Addr::from(<[u8; 16]>::try_from(address).map_err(|_| {
            nom::Err::Failure(make_error(input, nom::error::ErrorKind::LengthValue))
        })?);
        Ok((input, AAAA { address }))
    }

    pub(crate) fn serialize<const LK: usize>(&self, w: &mut Writer<'_, '_, LK>) {
        let len = 16u16.to_be_bytes();
        w[..2].copy_from_slice(&len);
        w.inc(2);
        w[..16].copy_from_slice(&self.address.octets());
        w.inc(16);
    }
}

// Struct for PTR record
#[derive(Debug, PartialEq, Eq)]
pub struct PTR<'a, const LLEN: usize> {
    pub name: Label<'a, LLEN>,
}

impl<'a, const LLEN: usize> PTR<'a, LLEN> {
    pub(crate) fn parse(input: &'a [u8], context: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("PTR::parse");
        let (input, _) = be_u16(input)?;
        let (input, name) = Label::parse(input, context)?;
        Ok((input, PTR { name }))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        let r = w.reserve(2);
        self.name.serialize(w);
        let len = w.distance_from_reservation(&r) - 2;
        w.write_reservation(r, &(len as u16).to_be_bytes());
    }
}

// Struct for TXT record
#[derive(Debug, PartialEq, Eq)]
pub struct TXT<'a> {
    pub text: &'a str,
}

impl<'a> TXT<'a> {
    pub(crate) fn parse(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("TXT::parse");
        let (input, text_len) = be_u16(input)?;
        let (input, text) = take(text_len)(input)?;
        let text = str::from_utf8(text).map_err(|_| {
            nom::Err::Failure(make_error(input, nom::error::ErrorKind::AlphaNumeric))
        })?;
        Ok((input, TXT { text }))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        let text_len = self.text.len() as u16;
        w[..2].copy_from_slice(&text_len.to_be_bytes());
        w.inc(2);
        w[..text_len as usize].copy_from_slice(self.text.as_bytes());
        w.inc(text_len as usize);
    }
}

// Struct for SRV record
#[derive(Debug, PartialEq, Eq)]
pub struct SRV<'a, const LLEN: usize> {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: Label<'a, LLEN>,
}

impl<'a, const LLEN: usize> SRV<'a, LLEN> {
    pub(crate) fn parse(input: &'a [u8], context: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("SRV::parse");
        let (input, _) = be_u16(input)?;
        let (input, priority) = be_u16(input)?;
        let (input, weight) = be_u16(input)?;
        let (input, port) = be_u16(input)?;
        let (input, target) = Label::parse(input, context)?;

        Ok((
            input,
            SRV {
                priority,
                weight,
                port,
                target,
            },
        ))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        let r = w.reserve(2);

        w[..2].copy_from_slice(&self.priority.to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&self.weight.to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&self.port.to_be_bytes());
        w.inc(2);

        self.target.serialize(w);

        let len = w.distance_from_reservation(&r) - 2;
        w.write_reservation(r, &(len as u16).to_be_bytes());
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for A {
    fn format(&self, fmt: defmt::Formatter) {
        use crate::format::FormatIpv4Addr;
        defmt::write!(fmt, "A({})", FormatIpv4Addr(self.address))
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for AAAA {
    fn format(&self, fmt: defmt::Formatter) {
        use crate::format::FormatIpv6Addr;
        defmt::write!(fmt, "AAAA({})", FormatIpv6Addr(self.address))
    }
}

#[cfg(feature = "defmt")]
impl<'a, const LLEN: usize> defmt::Format for Record<'a, LLEN> {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Record::A(record) => defmt::write!(fmt, "Record::A({:?})", record),
            Record::AAAA(record) => defmt::write!(fmt, "Record::AAAA({:?})", record),
            Record::PTR(record) => defmt::write!(fmt, "Record::PTR({:?})", record),
            Record::TXT(record) => defmt::write!(fmt, "Record::TXT({:?})", record),
            Record::SRV(record) => defmt::write!(fmt, "Record::SRV({:?})", record),
        }
    }
}

#[cfg(feature = "defmt")]
impl<'a, const LLEN: usize> defmt::Format for PTR<'a, LLEN> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "PTR {{ name: {:?} }}", self.name);
    }
}

#[cfg(feature = "defmt")]
impl<'a> defmt::Format for TXT<'a> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "TXT {{ text: {:?} }}", self.text);
    }
}

#[cfg(feature = "defmt")]
impl<'a, const LLEN: usize> defmt::Format for SRV<'a, LLEN> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "SRV {{ priority: {}, weight: {}, port: {}, target: {:?} }}",
            self.priority,
            self.weight,
            self.port,
            self.target
        );
    }
}
