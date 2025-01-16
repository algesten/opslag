use nom::number::complete::be_u32;
use nom::{number::complete::be_u16, IResult};

use super::records::Record;
use super::Label;
use crate::writer::Writer;

#[derive(Debug, PartialEq, Eq, defmt::Format)]
pub struct Query<'a, const LLEN: usize> {
    pub name: Label<'a, LLEN>,
    pub qtype: QType,
    pub qclass: QClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
#[repr(u16)]
pub enum QType {
    A = 1,
    AAAA = 28,
    PTR = 12,
    TXT = 16,
    SRV = 33,
    Any = 255,
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
#[repr(u16)]
pub enum QClass {
    IN = 1,
    Multicast = 32769, // (IN + Cache flush bit)
    Unknown(u16),
}

impl<'a, const LLEN: usize> Query<'a, LLEN> {
    pub(crate) fn parse(input: &'a [u8], context: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("Query::parse");
        let (input, name) = Label::parse(input, context)?;
        let (input, qtype) = be_u16(input)?;
        let qtype = QType::from_u16(qtype);
        let (input, qclass) = be_u16(input)?;
        let qclass = QClass::from_u16(qclass);
        Ok((
            input,
            Query {
                name,
                qtype,
                qclass,
            },
        ))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        self.name.serialize(w);
        w[..2].copy_from_slice(&self.qtype.to_u16().to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&self.qclass.to_u16().to_be_bytes());
        w.inc(2);
    }
}

#[derive(Debug, PartialEq, Eq, defmt::Format)]
pub struct Answer<'a, const LLEN: usize> {
    pub name: Label<'a, LLEN>,
    pub atype: QType,
    pub aclass: QClass,
    pub ttl: u32,
    pub record: Record<'a, LLEN>,
}

impl QType {
    pub fn from_u16(value: u16) -> Self {
        match value {
            1 => QType::A,
            28 => QType::AAAA,
            12 => QType::PTR,
            16 => QType::TXT,
            33 => QType::SRV,
            255 => QType::Any,
            _ => QType::Unknown(value),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            QType::A => 1,
            QType::AAAA => 28,
            QType::PTR => 12,
            QType::TXT => 16,
            QType::SRV => 33,
            QType::Any => 255,
            QType::Unknown(value) => *value,
        }
    }
}

impl QClass {
    pub fn from_u16(value: u16) -> Self {
        match value {
            1 => QClass::IN,
            32769 => QClass::Multicast,
            _ => QClass::Unknown(value),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            QClass::IN => 1,
            QClass::Multicast => 32769,
            QClass::Unknown(value) => *value,
        }
    }
}

impl<'a, const LLEN: usize> Answer<'a, LLEN> {
    pub(crate) fn parse(input: &'a [u8], context: &'a [u8]) -> IResult<&'a [u8], Self> {
        let (input, name) = Label::parse(input, context)?;
        let (input, atype) = be_u16(input)?;
        let atype = QType::from_u16(atype);
        let (input, aclass) = be_u16(input)?;
        let aclass = QClass::from_u16(aclass);

        let (input, ttl) = be_u32(input)?;
        let (input, record) = Record::parse(input, context, atype)?;
        Ok((
            input,
            Answer {
                name,
                atype,
                aclass,
                ttl,
                record,
            },
        ))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        self.name.serialize(w);
        w[..2].copy_from_slice(&self.atype.to_u16().to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&self.aclass.to_u16().to_be_bytes());
        w.inc(2);
        w[..4].copy_from_slice(&self.ttl.to_be_bytes());
        w.inc(4);
        self.record.serialize(w);
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::dns::records::A;
    use core::net::Ipv4Addr;

    #[test]
    fn roundtrip_query() {
        let name = Label::<4>::new("example.local");

        let query = Query {
            name,
            qtype: QType::A,
            qclass: QClass::IN,
        };

        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        query.serialize(&mut buffer);
        let (_, parsed_query) = Query::parse(buffer.into_inner(), &[1]).unwrap();

        assert_eq!(query, parsed_query);
    }

    #[test]
    fn roundtrip_answer() {
        let name = Label::new("example.local");

        let answer: Answer<4> = Answer {
            name,
            atype: QType::A,
            aclass: QClass::IN,
            ttl: 120,
            record: Record::A(A {
                address: Ipv4Addr::new(192, 168, 1, 1),
            }),
        };

        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        answer.serialize(&mut buffer);
        let (_, parsed_answer) = Answer::parse(buffer.into_inner(), &[1]).unwrap();

        assert_eq!(answer, parsed_answer);
    }
}
