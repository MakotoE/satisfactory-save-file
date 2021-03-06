#![no_main]
use libfuzzer_sys::fuzz_target;
use satisfactory_save_file::WorldProperties;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = WorldProperties::parse(s);
    }
});