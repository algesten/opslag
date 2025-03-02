use nom::error::make_error;
use nom::number::complete::be_u16;
use nom::IResult;

pub use flags::*;
pub use label::*;
pub use query::*;
pub use records::*;
pub use reqres::*;

use crate::writer::Writer;

mod flags;
mod label;
mod query;
mod records;
mod reqres;

#[derive(Debug, defmt::Format)]
pub enum Message<'a, const QLEN: usize, const ALEN: usize, const LLEN: usize> {
    Request(Request<'a, QLEN, LLEN>),
    Response(Response<'a, QLEN, ALEN, LLEN>),
}

impl<'a, const QLEN: usize, const ALEN: usize, const LLEN: usize> Message<'a, QLEN, ALEN, LLEN> {
    pub fn parse(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("Message::parse");
        if input.len() < 4 {
            warn!("Message::parse too small message: {}", input.len(),);
            return Err(nom::Err::Failure(make_error(
                input,
                nom::error::ErrorKind::LengthValue,
            )));
        }
        let (_, flags) = be_u16(&input[2..])?;
        if flags & 0x8000 == 0 {
            let (input, request) = Request::parse(input)?;
            Ok((input, Message::Request(request)))
        } else {
            let (input, response) = Response::parse(input)?;
            Ok((input, Message::Response(response)))
        }
    }

    pub fn serialize<'b, const LK: usize>(&self, output: &mut [u8]) -> usize {
        let mut w = Writer::<LK>::new(output);
        match self {
            Message::Request(v) => v.serialize(&mut w),
            Message::Response(v) => v.serialize(&mut w),
        }
        w.len()
    }
}

#[cfg(all(feature = "std", test))]
mod test {
    use crate::test::init_test_log;

    use super::*;

    #[test]
    fn parse_offset_label() {
        init_test_log();

        const FAIL: &[u8] = &[
            0, 0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 8, 67, 72, 49, 64, 105, 110, 45, 97, 14, 95, 110,
            101, 116, 97, 117, 100, 105, 111, 45, 99, 104, 97, 110, 4, 95, 117, 100, 112, 5, 108,
            111, 99, 97, 108, 0, 0, 33, 128, 1, 192, 12, 0, 16, 128, 1, 8, 67, 72, 50, 64, 105,
            110, 45, 97, 192, 21, 0, 33, 128, 1, 192, 58, 0, 16, 128, 1, 8, 67, 72, 49, 64, 105,
            110, 45, 98, 192, 21, 0, 33, 128, 1, 192, 79, 0, 16, 128, 1, 8, 67, 72, 50, 64, 105,
            110, 45, 98, 192, 21, 0, 33, 128, 1, 192, 100, 0, 16, 128, 1, 8, 67, 72, 49, 64, 105,
            110, 45, 99, 192, 21, 0, 33, 128, 1, 192, 121, 0, 16, 128, 1, 8, 67, 72, 50, 64, 105,
            110, 45, 99, 192, 21, 0, 33, 128, 1, 192, 142, 0, 16, 128, 1,
        ];

        let m = Message::<12, 12, 4>::parse(FAIL).unwrap();

        println!("{:#?}", m);
    }

    #[test]
    fn parse_recursive_label() {
        init_test_log();

        const FAIL: &[u8] = &[
            6, 0, 0, 0, 1, 1, 162, 8, 0, 1, 0, 10, 1, 14, 1, 1, 1, 1, 64, 64, 64, 64, 64, 64, 85,
            0, 1, 0, 10, 1, 14, 64, 64, 64, 40, 64, 64, 64, 64, 64, 64, 64, 64, 64, 208, 64, 64,
            64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 1, 0, 0, 0,
            64, 64,
        ];

        Message::<12, 12, 4>::parse(FAIL).unwrap_err();
    }

    #[test]
    fn parse_recursive_label2() {
        init_test_log();

        const FAIL: &[u8] = &[
            14, 10, 0, 142, 10, 78, 44, 10, 0, 192, 192, 192, 192, 64, 64, 64, 40, 64, 96, 64, 64,
            64, 64, 64, 64, 64, 208, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 173, 173, 173,
            173, 173, 173, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 1, 0, 0, 1, 64,
            64, 64, 14, 10, 16, 241, 8, 211, 0, 0, 0, 0, 0, 0, 0, 64, 64, 64, 64, 0, 0, 0, 0,
        ];

        Message::<12, 12, 4>::parse(FAIL).unwrap_err();
    }
}
