#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    opslag::dns::Message::parse(data).ok();
});
