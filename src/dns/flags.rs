use core::fmt;
use nom::number::complete::be_u16;
use nom::IResult;

use crate::writer::Writer;

#[derive(Default, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct Flags(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Opcode {
    Query = 0,
    IQuery = 1,
    Status = 2,
    Reserved = 3,
    Notify = 4,
    Update = 5,
    // Other values are reserved
}

impl From<u8> for Opcode {
    fn from(value: u8) -> Self {
        match value {
            0 => Opcode::Query,
            1 => Opcode::IQuery,
            2 => Opcode::Status,
            4 => Opcode::Notify,
            5 => Opcode::Update,
            _ => Opcode::Reserved,
        }
    }
}

impl From<Opcode> for u8 {
    fn from(opcode: Opcode) -> Self {
        opcode as u8
    }
}

impl Flags {
    fn new() -> Self {
        Flags(0)
    }

    pub fn standard_request() -> Self {
        let mut flags = Flags::new();
        flags.set_query(true);
        flags.set_opcode(Opcode::Query);
        flags.set_recursion_desired(true);
        flags
    }

    pub fn standard_response() -> Self {
        let mut flags = Flags::new();
        flags.set_query(false);
        flags.set_opcode(Opcode::Query);
        flags.set_authoritative(true);
        flags.set_recursion_available(false);
        flags
    }

    // QR: Query/Response Flag
    pub fn is_query(&self) -> bool {
        (self.0 & 0x8000) == 0
    }

    pub fn set_query(&mut self, is_query: bool) {
        if is_query {
            self.0 &= !0x8000;
        } else {
            self.0 |= 0x8000;
        }
    }

    // Opcode (bits 1-4)
    pub fn get_opcode(&self) -> Opcode {
        Opcode::from(((self.0 >> 11) & 0x0F) as u8)
    }

    pub fn set_opcode(&mut self, opcode: Opcode) {
        self.0 = (self.0 & !0x7800) | ((u8::from(opcode) as u16 & 0x0F) << 11);
    }

    // AA: Authoritative Answer
    pub fn is_authoritative(&self) -> bool {
        (self.0 & 0x0400) != 0
    }

    pub fn set_authoritative(&mut self, authoritative: bool) {
        if authoritative {
            self.0 |= 0x0400;
        } else {
            self.0 &= !0x0400;
        }
    }

    // TC: Truncated
    pub fn is_truncated(&self) -> bool {
        (self.0 & 0x0200) != 0
    }

    pub fn set_truncated(&mut self, truncated: bool) {
        if truncated {
            self.0 |= 0x0200;
        } else {
            self.0 &= !0x0200;
        }
    }

    // RD: Recursion Desired
    pub fn is_recursion_desired(&self) -> bool {
        (self.0 & 0x0100) != 0
    }

    pub fn set_recursion_desired(&mut self, recursion_desired: bool) {
        if recursion_desired {
            self.0 |= 0x0100;
        } else {
            self.0 &= !0x0100;
        }
    }

    // RA: Recursion Available
    pub fn is_recursion_available(&self) -> bool {
        (self.0 & 0x0080) != 0
    }

    pub fn set_recursion_available(&mut self, recursion_available: bool) {
        if recursion_available {
            self.0 |= 0x0080;
        } else {
            self.0 &= !0x0080;
        }
    }

    // Z: Reserved for future use (bits 9-11)
    pub fn get_reserved(&self) -> u8 {
        ((self.0 >> 4) & 0x07) as u8
    }

    pub fn set_reserved(&mut self, reserved: u8) {
        self.0 = (self.0 & !0x0070) | ((reserved as u16 & 0x07) << 4);
    }

    // RCODE: Response Code (bits 12-15)
    pub fn get_rcode(&self) -> u8 {
        (self.0 & 0x000F) as u8
    }

    pub fn set_rcode(&mut self, rcode: u8) {
        self.0 = (self.0 & !0x000F) | (rcode as u16 & 0x0F);
    }

    pub fn parse(input: &[u8]) -> IResult<&[u8], Flags> {
        let (input, flags) = be_u16(input)?;
        Ok((input, Flags(flags)))
    }

    pub fn serialize<'a, 'b, const LK: usize>(&self, w: &mut Writer<'a, 'b, LK>) {
        w[..2].copy_from_slice(&self.0.to_be_bytes());
        w.inc(2);
    }
}

impl fmt::Debug for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flags")
            .field("query", &self.is_query())
            .field("opcode", &self.get_opcode())
            .field("authoritative", &self.is_authoritative())
            .field("truncated", &self.is_truncated())
            .field("recursion_desired", &self.is_recursion_desired())
            .field("recursion_available", &self.is_recursion_available())
            .field("reserved", &self.get_reserved())
            .field("rcode", &self.get_rcode())
            .finish()
    }
}
