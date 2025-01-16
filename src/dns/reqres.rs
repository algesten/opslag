use nom::{error::make_error, number::complete::be_u16, IResult};

use super::flags::Flags;
use super::query::{Answer, Query};
use crate::vec::Vec;
use crate::writer::Writer;

const ZERO_U16: [u8; 2] = 0u16.to_be_bytes();

#[derive(Debug, PartialEq, Eq, defmt::Format)]
pub struct Request<'a, const QLEN: usize, const LLEN: usize> {
    pub id: u16,
    pub flags: Flags,
    pub(crate) queries: Vec<Query<'a, LLEN>, QLEN>,
}

impl<'a, const QLEN: usize, const LLEN: usize> Request<'a, QLEN, LLEN> {
    pub fn parse(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("Request::parse");
        let context = input;
        let (input, id) = be_u16(input)?;
        let (input, flags) = Flags::parse(input)?;
        let (input, qdcount) = be_u16(input)?;
        let (input, _ancount) = be_u16(input)?;
        let (input, _nscount) = be_u16(input)?;
        let (input, _arcount) = be_u16(input)?;
        let mut queries = Vec::new();
        let mut input = input;
        for _ in 0..qdcount {
            let (new_input, query) = Query::parse(input, context)?;
            input = new_input;
            queries.push(query).map_err(|_| {
                debug!("Request::parse too many queries: {}", qdcount);
                nom::Err::Failure(make_error(input, nom::error::ErrorKind::TooLarge))
            })?;
        }
        Ok((input, Request { id, flags, queries }))
    }

    pub fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        w[..2].copy_from_slice(&self.id.to_be_bytes());
        w.inc(2);
        self.flags.serialize(w);
        w[..2].copy_from_slice(&(self.queries.len() as u16).to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&ZERO_U16); // ANCOUNT
        w.inc(2);
        w[..2].copy_from_slice(&ZERO_U16); // NSCOUNT
        w.inc(2);
        w[..2].copy_from_slice(&ZERO_U16); // ARCOUNT
        w.inc(2);
        for query in self.queries.iter() {
            query.serialize(w);
        }
    }
}

#[derive(Debug, PartialEq, Eq, defmt::Format)]
pub struct Response<'a, const QLEN: usize, const ALEN: usize, const LLEN: usize> {
    pub id: u16,
    pub flags: Flags,
    pub queries: Vec<Query<'a, LLEN>, QLEN>,
    pub answers: Vec<Answer<'a, LLEN>, ALEN>,
}

impl<'a, const QLEN: usize, const ALEN: usize, const LLEN: usize> Response<'a, QLEN, ALEN, LLEN> {
    pub fn parse(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("Response::parse");
        let context = input;
        let (input, id) = be_u16(input)?;
        let (input, flags) = Flags::parse(input)?;
        let (input, qdcount) = be_u16(input)?;
        let (input, ancount) = be_u16(input)?;
        let (input, _nscount) = be_u16(input)?;
        let (input, _arcount) = be_u16(input)?;

        let mut queries = Vec::new();
        let mut input = input;
        for _ in 0..qdcount {
            let (new_input, query) = Query::parse(input, context)?;
            input = new_input;
            queries.push(query).map_err(|_| {
                debug!("Response::parse too many queries: {}", qdcount);
                nom::Err::Failure(make_error(input, nom::error::ErrorKind::TooLarge))
            })?;
        }

        let mut answers = Vec::new();
        for _ in 0..ancount {
            let (new_input, answer) = Answer::parse(input, context)?;
            input = new_input;
            answers.push(answer).map_err(|_| {
                debug!("Response::parse too many answers: {}", ancount);
                nom::Err::Failure(make_error(input, nom::error::ErrorKind::TooLarge))
            })?;
        }
        Ok((
            input,
            Response {
                id,
                flags,
                queries,
                answers,
            },
        ))
    }

    pub fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        w[..2].copy_from_slice(&self.id.to_be_bytes());
        w.inc(2);
        self.flags.serialize(w);
        w[..2].copy_from_slice(&(self.queries.len() as u16).to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&(self.answers.len() as u16).to_be_bytes());
        w.inc(2);
        w[..2].copy_from_slice(&ZERO_U16); // NSCOUNT
        w.inc(2);
        w[..2].copy_from_slice(&ZERO_U16); // ARCOUNT
        w.inc(2);
        for query in self.queries.iter() {
            query.serialize(w);
        }
        for answer in self.answers.iter() {
            answer.serialize(w);
        }
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;
    use crate::dns::query::{QClass, QType};
    use crate::dns::{Label, Record, A, PTR, SRV, TXT};
    use crate::test::init_test_log;
    use core::net::Ipv4Addr;

    #[test]
    fn parse_query() {
        let data = [
            0xAA, 0xAA, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x65,
            // example . com in label format
            0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D, 0x00, //
            //
            0x00, 0x01, 0x00, 0x01,
        ];

        let (_, request) = Request::<12, 4>::parse(&data).unwrap();

        assert_eq!(request.id, 0xAAAA);
        assert_eq!(request.flags.0, 0x0100);
        assert_eq!(request.queries.len(), 1);
        assert_eq!(request.queries[0].name.to_string(), "example.com");
        assert_eq!(request.queries[0].qtype, QType::A);
        assert_eq!(request.queries[0].qclass, QClass::IN);
    }

    #[test]
    fn parse_response() {
        let data = [
            0xAA, 0xAA, // transaction ID
            0x81, 0x80, // flags
            0x00, 0x01, // 1 question
            0x00, 0x01, // 1 A-answer
            0x00, 0x00, // no authority
            0x00, 0x00, // no additional answers
            // example . com in label format
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D, 0x00, //
            //
            0x00, 0x01, 0x00, 0x01, //
            //
            0xC0, 0x0C, // ptr to question section
            //
            0x00, 0x01, 0x00, 0x01, // A and IN
            //
            0x00, 0x00, 0x00, 0x3C, // TTL 60 seconds
            //
            0x00, 0x04, // length of address
            // IP address:
            192, 168, 1, 3,
        ];

        let (_, response) = Response::<12, 12, 4>::parse(&data).unwrap();

        assert_eq!(response.id, 0xAAAA);
        assert_eq!(response.flags.0, 0x8180);
        assert_eq!(response.answers.len(), 1);
        assert_eq!(response.answers[0].name.to_string(), "example.com");
        assert_eq!(response.answers[0].atype, QType::A);
        assert_eq!(response.answers[0].aclass, QClass::IN);
        assert_eq!(response.answers[0].ttl, 60);
        if let Record::A(a) = &response.answers[0].record {
            assert_eq!(a.address, Ipv4Addr::new(192, 168, 1, 3));
        } else {
            panic!("Expected A record");
        }
    }

    #[test]
    fn parse_response_two_records() {
        let data = [
            0xAA, 0xAA, //
            0x81, 0x80, //
            0x00, 0x01, //
            0x00, 0x02, //
            0x00, 0x00, //
            0x00, 0x00, //
            // example . com in label format
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D, 0x00, //
            //
            0x00, 0x01, // query type
            0x00, 0x01, // query class
            //
            0xC0, 0x0C, // pointer
            0x00, 0x01, //
            0x00, 0x01, //
            0x00, 0x00, 0x00, 0x3C, // ttl 60 seconds
            0x00, 0x04, // length of A-record
            0x5D, 0xB8, 0xD8, 0x22, // a-record
            //
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D, 0x00, //
            //
            0x00, 0x10, // TXT
            0x00, 0x01, // IN
            //
            0x00, 0x00, 0x00, 0x3C, // ttl 60 seconds
            //
            0x00, 0x0F, // length of txt record
            // "test txt record"
            0x74, 0x65, 0x73, 0x74, 0x20, 0x74, 0x78, 0x74, 0x20, 0x72, 0x65, 0x63, 0x6F, 0x72,
            0x64,
        ];

        let (_, response) = Response::<12, 12, 4>::parse(&data).unwrap();

        assert_eq!(response.id, 0xAAAA);
        assert_eq!(response.flags.0, 0x8180);
        assert_eq!(response.answers.len(), 2);

        // First answer
        assert_eq!(response.answers[0].name.to_string(), "example.com");
        assert_eq!(response.answers[0].atype, QType::A);
        assert_eq!(response.answers[0].aclass, QClass::IN);
        assert_eq!(response.answers[0].ttl, 60);
        if let Record::A(a) = &response.answers[0].record {
            assert_eq!(a.address, Ipv4Addr::new(93, 184, 216, 34));
        } else {
            panic!("Expected A record");
        }

        // Second answer
        assert_eq!(response.answers[1].name.to_string(), "example.com");
        assert_eq!(response.answers[1].atype, QType::TXT);
        assert_eq!(response.answers[1].aclass, QClass::IN);
        assert_eq!(response.answers[1].ttl, 60);
        if let Record::TXT(txt) = &response.answers[1].record {
            assert_eq!(txt.text, "test txt record");
        } else {
            panic!("Expected TXT record");
        }
    }

    #[test]
    fn parse_response_srv() {
        let data = [
            //
            0xAA, 0xAA, // id
            0x81, 0x80, // flags
            0x00, 0x01, // one question
            0x00, 0x01, // one answer
            0x00, 0x00, // no authority
            0x00, 0x00, // no extra
            //
            0x04, 0x5f, 0x73, 0x69, 0x70, 0x04, 0x5f, 0x74, 0x63, 0x70, 0x07, 0x65, 0x78, 0x61,
            0x6d, 0x70, 0x6c, 0x65, 0x03, 0x63, 0x6f, 0x6d, 0x00, //
            //
            0x00, 0x21, // type SRV
            0x00, 0x01, // IN
            //
            0xc0, 0x0c, //
            //
            0x00, 0x21, // SRV
            0x00, 0x01, // IN
            0x00, 0x00, 0x00, 0x3C, // ttl 60
            //
            0x00, 0x19, // data len
            0x00, 0x0A, // prio
            0x00, 0x05, // weight
            0x13, 0xC4, // PORT
            //
            0x09, 0x73, 0x69, 0x70, 0x73, 0x65, 0x72, 0x76, 0x65, 0x72, 0x07, 0x65, 0x78, 0x61,
            0x6d, 0x70, 0x6c, 0x65, 0x03, 0x63, 0x6f, 0x6d, 0x00, //
        ];

        let (_, response) = Response::<12, 12, 4>::parse(&data).unwrap();

        assert_eq!(response.id, 0xAAAA);
        assert_eq!(response.flags.0, 0x8180);
        assert_eq!(response.answers.len(), 1);

        // Answer
        assert_eq!(
            response.answers[0].name.to_string(),
            "_sip._tcp.example.com"
        );
        assert_eq!(response.answers[0].atype, QType::SRV);
        assert_eq!(response.answers[0].aclass, QClass::IN);
        assert_eq!(response.answers[0].ttl, 60);
        let Record::SRV(srv) = &response.answers[0].record else {
            panic!("Expected SRV record");
        };

        assert_eq!(srv.priority, 10);
        assert_eq!(srv.weight, 5);
        assert_eq!(srv.port, 5060);
        assert_eq!(srv.target.to_string(), "sipserver.example.com");
    }

    #[test]
    fn parse_response_back_forth() {
        init_test_log();

        let data = [
            0, 0, // Transaction ID
            132, 0, // Response, Authoritative Answer, No Recursion
            0, 0, // 0 questions
            0, 4, // 4 answers
            0, 0, // 0 authority RRs
            0, 0, // 0 additional RRs
            // _midiriff
            9, 95, 109, 105, 100, 105, 114, 105, 102, 102, //
            // _udp
            4, 95, 117, 100, 112, //
            // local
            5, 108, 111, 99, 97, 108, //
            0,   // <end>
            //
            0, 12, // PTR
            0, 1, // Class IN
            0, 0, 17, 148, // TTL 6036 seconds
            0, 10, // Data Length 10
            // pi35291
            7, 112, 105, 51, 53, 50, 57, 49, //
            //
            192, 12, // Pointer to _midirif._udp._local.
            //
            192, 44, // Pointer to instace name: pi35291._midirif._udp._local.
            0, 33, // SRV
            128, 1, // IN (Cache flush bit set)
            0, 0, 0, 120, // TTL 120 seconds
            0, 11, // Data Length 11
            0, 0, // Priority 0
            0, 0, // Weight 0
            137, 219, // Port 35291
            2, 112, 105, // _pi
            192, 27, // Pointer to: .local.
            //
            192, 44, 0, 16, 128, 1, 0, 0, 17, 148, 0, 1, 0, 192, 72, 0, 1, 128, 1, 0, 0, 0, 120, 0,
            4, 10, 1, 1, 9,
        ];

        let (_, response) = Response::<12, 12, 4>::parse(&data).unwrap();

        println!("{:#?}", response);

        assert_eq!(response.answers[0].name, "_midiriff._udp.local");
        // assert_eq!(response.answers[0].ttl, 120);
        let Record::PTR(ptr) = &response.answers[0].record else {
            panic!()
        };
        assert_eq!(ptr.name, "pi35291._midiriff._udp.local");

        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        response.serialize(&mut buffer);

        let buffer = buffer.into_inner();
        println!("{:?}", buffer);

        let (_, response2) = Response::<12, 12, 4>::parse(buffer).unwrap();

        assert_eq!(response, response2);
    }

    #[test]
    fn mdns_service_response() {
        init_test_log();

        let mut response = Response::<1, 4, 4> {
            id: 0x1234,
            flags: Flags::standard_response(),
            queries: Vec::new(),
            answers: Vec::new(),
        };

        let query = Query {
            name: Label::new("_test._udp.local"),
            qtype: QType::PTR,
            qclass: QClass::IN,
        };
        response.queries.push(query).unwrap();

        let ptr_answer = Answer {
            name: Label::new("_test._udp.local"),
            atype: QType::PTR,
            aclass: QClass::IN,
            ttl: 4500,
            record: Record::PTR(PTR {
                name: Label::new("test-service._test._udp.local"),
            }),
        };
        response.answers.push(ptr_answer).unwrap();

        let srv_answer = Answer {
            name: Label::new("test-service._test._udp.local"),
            atype: QType::SRV,
            aclass: QClass::IN,
            ttl: 120,
            record: Record::SRV(SRV {
                priority: 0,
                weight: 0,
                port: 8080,
                target: Label::new("host.local"),
            }),
        };
        response.answers.push(srv_answer).unwrap();

        let txt_answer = Answer {
            name: Label::new("test-service._test._udp.local"),
            atype: QType::TXT,
            aclass: QClass::IN,
            ttl: 120,
            record: Record::TXT(TXT { text: "path=/test" }),
        };
        response.answers.push(txt_answer).unwrap();

        let a_answer = Answer {
            name: Label::new("host.local"),
            atype: QType::A,
            aclass: QClass::IN,
            ttl: 120,
            record: Record::A(A {
                address: Ipv4Addr::new(192, 168, 1, 100),
            }),
        };
        response.answers.push(a_answer).unwrap();

        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        response.serialize(&mut buffer);

        let buffer = buffer.into_inner();

        let (_, parsed_response) = Response::<1, 4, 4>::parse(buffer).unwrap();

        assert_eq!(response, parsed_response);
    }
}
