#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 — the parser takes &str
    if let Ok(input) = std::str::from_utf8(data) {
        // Must never panic, regardless of input
        let _ = seuil::parser::parse(input);
    }
});
