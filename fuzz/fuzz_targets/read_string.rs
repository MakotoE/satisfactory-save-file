#![no_main]
use libfuzzer_sys::fuzz_target;
use satisfactory_save_file::read_string;

fuzz_target!(|data: &[u8]| {
    let _ = read_string(&mut &data.to_vec()[..]);
});