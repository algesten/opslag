use core::fmt;
use core::str;

use nom::bytes::complete::take;
use nom::error::make_error;
use nom::number::complete::be_u8;
use nom::IResult;

use crate::vec::Vec;
use crate::writer::Writer;

#[derive(Default, Clone)]
pub struct Label<'a, const LLEN: usize> {
    items: Vec<LabelPart<'a>, LLEN>,
}

#[derive(Clone)]
enum LabelPart<'a> {
    Run(LabelRun<'a>),
    Str(LabelStr<'a>),
}

impl<'a> LabelPart<'a> {
    fn iter(&self) -> LabelPartIter<'a> {
        match self {
            LabelPart::Run(run) => LabelPartIter::Run(run.iter()),
            LabelPart::Str(lab) => LabelPartIter::Str(lab.iter()),
        }
    }

    fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>, is_last: bool) {
        match self {
            LabelPart::Run(run) => run.serialize(w),
            LabelPart::Str(lab) => lab.serialize(w, is_last),
        }
    }
}

/// One segment of label.
///
/// I.e a run of `<len>-<label>, <len>-<label>, <len>-<label>, <len>-<label>`
///
/// If len is an offset into the context, this forms a new LabelRun.
#[derive(Default, Clone)]
struct LabelRun<'a> {
    run: &'a [u8],
    context: &'a [u8],
}

impl<'a> LabelRun<'a> {
    fn do_serialize(&self, output: &mut [u8]) -> usize {
        let mut offset = 0;
        let mut data = self.run;

        while !data.is_empty() {
            let len = data[0] as usize;
            if len == 0 {
                output[offset] = 0;
                offset += 1;
                break;
            } else if len & 0xc0 > 0 {
                let offset = (len & 0x3f) << 8 | (data[1] as usize);
                data = &self.context[offset..];
            } else {
                output[offset..offset + len + 1].copy_from_slice(&data[..len + 1]);
                offset += len + 1;
                data = &data[len + 1..];
            }
        }

        offset
    }

    fn serialize<const LK: usize>(&self, w: &mut Writer<'a, '_, LK>) {
        let n = self.do_serialize(&mut w[..]);
        w.inc(n);
    }

    fn iter(&self) -> LabelRunIter<'a> {
        LabelRunIter {
            data: self.run,
            context: self.context,
            partial: None,
        }
    }
}

#[derive(Clone)]
struct LabelStr<'a>(&'a str);

impl<'a> LabelStr<'a> {
    fn new(s: &'a str) -> Self {
        LabelStr(s)
    }

    fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>, is_last: bool) {
        serialize_str(self.0, w, is_last)
    }

    fn iter(&self) -> LabelStrIter<'a> {
        LabelStrIter { data: self.0 }
    }
}

impl<'a, const LLEN: usize> Label<'a, LLEN> {
    pub fn new(s: &'a str) -> Self {
        assert!(!s.ends_with('.'), "Labels must not end with: .");
        let mut l = Label::default();
        l.push_back(s);
        l
    }

    pub fn push_front(&mut self, part: &'a str) -> bool {
        self.items
            .insert(0, LabelPart::Str(LabelStr::new(part)))
            .is_ok()
    }

    pub fn push_back(&mut self, part: &'a str) -> bool {
        self.items.push(LabelPart::Str(LabelStr::new(part))).is_ok()
    }

    pub(crate) fn parse(input: &'a [u8], context: &'a [u8]) -> IResult<&'a [u8], Self> {
        trace!("Label::parse start");
        assert!(!context.is_empty());
        let mut label = Label::default();
        let (input, _) = Self::do_parse(input, context, &mut label, 4)?;
        Ok((input, label))
    }

    fn do_parse(
        input: &'a [u8],
        context: &'a [u8],
        into: &mut Label<'a, LLEN>,
        recurse_limit: u8,
    ) -> IResult<&'a [u8], ()> {
        let all = input;
        let mut input = input;
        let mut run_end = 0;
        loop {
            let (new_input, len) = be_u8(input)?;
            let is_end = len == 0;
            let is_ptr = len & 0xc0 > 0;

            if !is_ptr {
                run_end += 1;
            }

            if is_end || (is_ptr && run_end > 0) {
                // That's the end of the current run.
                into.items
                    .push(LabelPart::Run(LabelRun {
                        run: &all[..run_end],
                        context,
                    }))
                    .map_err(|_| {
                        warn!("Label::parse too many parts");
                        nom::Err::Failure(make_error(input, nom::error::ErrorKind::TooLarge))
                    })?;
            }

            if is_end {
                trace!("Label::parse end: {:?}", into);
                input = new_input;
                break;
            }

            if is_ptr {
                trace!("Label::parse from offset");
                let (new_input, b) = be_u8(new_input)?;
                // pointer into context.
                let offset = ((len & 0x3f) as usize) << 8 | (b as usize);
                let Some(pointered) = context.get(offset..) else {
                    warn!(
                        "Label::parse offset wrong: {} in len: {}",
                        offset,
                        context.len()
                    );
                    return Err(nom::Err::Failure(make_error(
                        input,
                        nom::error::ErrorKind::LengthValue,
                    )));
                };

                if pointered.len() < 2 || pointered[..2] == input[..2] || recurse_limit == 0 {
                    warn!("Label::parse offset recurses",);
                    return Err(nom::Err::Failure(make_error(
                        input,
                        nom::error::ErrorKind::LengthValue,
                    )));
                }

                trace!("Label::parse ptr({}) after: {:?}", offset, into);
                let (_, _) = Self::do_parse(pointered, context, into, recurse_limit - 1)?;
                input = new_input;
                break;
            }

            // regular label
            let (new_input, label) = take(len)(new_input)?;

            // Verify it's correct utf8
            str::from_utf8(label).map_err(|_| {
                nom::Err::Failure(make_error(input, nom::error::ErrorKind::AlphaNumeric))
            })?;

            input = new_input;
            run_end += len as usize;
        }

        Ok((input, ()))
    }

    pub(crate) fn serialize<'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        let mut iter = self.items.iter().peekable();
        loop {
            let Some(label) = iter.next() else {
                break;
            };
            let is_last = iter.peek().is_none();
            label.serialize(w, is_last);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.items.iter().flat_map(|part| part.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

fn serialize_str<'a, const LK: usize>(v: &'a str, w: &mut Writer<'a, '_, LK>, is_last: bool) {
    let mut rest = v;

    while !rest.is_empty() {
        if is_last {
            if let Some(pos) = w.find_label(rest) {
                let pointer = 0xc000 | pos as u16;
                w[..2].copy_from_slice(&pointer.to_be_bytes());
                w.inc(2);
                return;
            }
        }

        let next = rest.split('.').next().unwrap();
        let n = serialize_str_single(next, &mut w[..]);
        w.push_label(rest, 0);
        w.inc(n);

        if let Some(pos) = rest.find('.') {
            rest = &rest[pos + 1..];
        } else {
            break;
        }
    }

    if is_last {
        w[0] = 0;
        w.inc(1);
    }
}

fn serialize_str_single(v: &str, output: &mut [u8]) -> usize {
    let len = v.len();
    output[0] = len as u8;
    let output = &mut output[1..];
    output[..len].copy_from_slice(v.as_bytes());
    1 + len
}

impl<const LLEN: usize> fmt::Display for Label<'_, LLEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.items.is_empty() {
            return Ok(());
        }
        let last = self.items.len() - 1;
        for (i, item) in self.items.iter().enumerate() {
            if i != last {
                core::write!(f, "{}.", item)?;
            } else {
                core::write!(f, "{}", item)?;
            }
        }
        Ok(())
    }
}

struct LabelRunIter<'a> {
    data: &'a [u8],
    context: &'a [u8],
    partial: Option<&'a str>,
}

impl<'a> Iterator for LabelRunIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(partial) = self.partial.take() {
            if let Some((part, rest)) = partial.split_once('.') {
                self.partial = Some(rest);
                return Some(part);
            } else {
                return Some(partial);
            }
        }

        if self.data.len() < 2 {
            return None;
        }

        let len = self.data[0] as usize;
        if len & 0xc0 > 0 {
            let offset = (len & 0x3f) << 8 | (self.data[1] as usize);
            self.data = &self.context[offset..];
            self.next()
        } else {
            let bytes = &self.data[1..1 + len];
            // invariant: the LabelRun is only constructed via parse(), which
            // validates the utf8 as part of parsing.
            let s = core::str::from_utf8(bytes).unwrap();

            self.data = &self.data[1 + len..];

            if let Some((part, rest)) = s.split_once('.') {
                self.partial = Some(rest);
                Some(part)
            } else {
                Some(s)
            }
        }
    }
}

struct LabelStrIter<'a> {
    data: &'a str,
}

impl<'a> Iterator for LabelStrIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        let end = self.data.find('.').unwrap_or(self.data.len());

        let part = &self.data[..end];
        self.data = if end == self.data.len() {
            ""
        } else {
            &self.data[end + 1..] // Skip the dot
        };
        Some(part)
    }
}

impl fmt::Display for LabelPart<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.iter();
        if let Some(first) = iter.next() {
            core::write!(f, "{}", first)?;
            for part in iter {
                core::write!(f, ".{}", part)?;
            }
        }
        Ok(())
    }
}

impl Default for LabelPart<'_> {
    fn default() -> Self {
        Self::Str(LabelStr(""))
    }
}

impl PartialEq for LabelPart<'_> {
    fn eq(&self, other: &Self) -> bool {
        let mut self_iter = self.iter();
        let mut other_iter = other.iter();
        loop {
            match (self_iter.next(), other_iter.next()) {
                (Some(self_part), Some(other_part)) => {
                    if self_part != other_part {
                        return false;
                    }
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

impl Eq for LabelPart<'_> {}

// glue code
enum LabelPartIter<'a> {
    Run(LabelRunIter<'a>),
    Str(LabelStrIter<'a>),
}

impl<'a> Iterator for LabelPartIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            LabelPartIter::Run(iter) => iter.next(),
            LabelPartIter::Str(iter) => iter.next(),
        }
    }
}

impl<const LLEN: usize> PartialEq for Label<'_, LLEN> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<const LLEN: usize> PartialEq<&str> for Label<'_, LLEN> {
    fn eq(&self, other: &&str) -> bool {
        let mut self_iter = self.iter();
        let mut other_iter = other.split('.');

        loop {
            let (s1, s2) = (self_iter.next(), other_iter.next());
            match (s1, s2) {
                (Some(self_part), Some(other_part)) => {
                    if self_part != other_part {
                        return false;
                    }
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

impl<const LLEN: usize> Eq for Label<'_, LLEN> {}

impl<const LLEN: usize> fmt::Debug for Label<'_, LLEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        core::write!(f, "Label(\"")?;
        let mut iter = self.iter();
        if let Some(first) = iter.next() {
            core::write!(f, "{}", first)?;
            for part in iter {
                core::write!(f, ".{}", part)?;
            }
        }
        core::write!(f, "\")")?;
        Ok(())
    }
}

impl<const LLEN: usize> defmt::Format for Label<'_, LLEN> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Label(\"");
        let mut iter = self.iter();
        if let Some(first) = iter.next() {
            defmt::write!(fmt, "{}", first);
            for part in iter {
                defmt::write!(fmt, ".{}", part);
            }
        }
        defmt::write!(fmt, "\")");
    }
}

#[cfg(all(feature = "std", test))]
mod test {
    use crate::test::init_test_log;

    use super::*;

    #[test]
    fn static_label() {
        let _static: Label<'static, 4> = Label::new("example.local");
    }

    #[test]
    fn serialize_str_label() {
        let label: Label<'static, 4> = Label::new("_service._udp.local");
        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        label.serialize(&mut buffer);
        assert_eq!(buffer.into_inner(), b"\x08_service\x04_udp\x05local\x00");
    }

    #[test]
    fn parse_and_serialize_label_with_context() {
        init_test_log();

        let data = [
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D,
            0x00, // example.com
            0xC0, 0x00, // pointer to the start of the label
        ];
        let context = &data[..];

        let (_, label) = Label::<4>::parse(&data[13..], context).unwrap();

        let mut buffer = [0u8; 256];
        let mut buffer = Writer::<10>::new(&mut buffer);
        label.serialize(&mut buffer);
        assert_eq!(buffer.into_inner(), b"\x07example\x03com\x00");
    }

    #[test]
    fn parse_and_create_label() {
        let data = [
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D,
            0x00, // example.com
            0xC0, 0x00, // pointer to the start of the label
        ];
        let context = &data[..];

        let (_, parsed_label) = Label::<4>::parse(&data[13..], context).unwrap();

        let mut created_label = Label::<4>::new("example");
        created_label.push_back("com");

        assert_eq!(parsed_label, created_label);
    }

    #[test]
    fn label_eq_str_parsed() {
        let data = [
            0x07, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x03, 0x63, 0x6F, 0x6D,
            0x00, // example.com
            0xC0, 0x00, // pointer to the start of the label
        ];
        let context = &data[..];

        let (_, parsed_label) = Label::<4>::parse(&data[13..], context).unwrap();

        assert_eq!(parsed_label, "example.com");
    }

    #[test]
    fn label_eq_str_created() {
        let mut created_label = Label::<4>::new("example");
        created_label.push_back("com");

        assert_eq!(created_label, "example.com");
    }

    #[test]
    fn default_label_is_empty() {
        let label: Label<4> = Label::default();
        assert!(label.is_empty());
    }

    #[test]
    fn label_new_without_dot_is_not_empty() {
        let label: Label<4> = Label::new("example");
        assert!(!label.is_empty());
    }
}
