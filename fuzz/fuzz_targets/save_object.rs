#![no_main]
use libfuzzer_sys::fuzz_target;
use satisfactory_save_file::SaveObject;

fuzz_target!(|data: &[u8]| {
    let _ = SaveObject::parse(&mut &data.to_vec()[..]);
});