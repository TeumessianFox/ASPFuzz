/// Generate initial inputs for the fuzzer based on provided UEFI images

use libafl_qemu::GuestAddr;
use std::fs;
use std::path::{
    Path,
    PathBuf,
};

pub struct InitialInput {}

impl InitialInput {
    pub fn new() -> Self {
        Self {}
    }

    pub fn create_initial_inputs(
        &self,
        flash_base: &Vec<String>,
        input_mem: &Vec<(GuestAddr, usize)>,
        flash_size: GuestAddr,
        input_total_size: usize,
        input_dir: PathBuf,
    ) -> PathBuf {
        if flash_base.is_empty() {
            let mut new_input_path = PathBuf::from(&input_dir);
            new_input_path.push("input0000");
            fs::write(new_input_path, vec![0; input_total_size]).unwrap();
        }
        for (i, base) in flash_base.iter().enumerate() {
            let mut new_input_image = Vec::<u8>::new();
            let image: Vec<u8> = fs::read(Path::new(base)).unwrap();
            for mem in input_mem.iter() {
                assert!(mem.0 < flash_size && (mem.1 as GuestAddr) < flash_size, "Memory region outsize of flash memory size");
                let mem_section = &image[((mem.0 & 0x00FF_FFFF) as usize)..((mem.0 & 0x00FF_FFFF) as usize)+mem.1];
                new_input_image.extend_from_slice(mem_section);
            }
            if input_total_size != new_input_image.len() {
                panic!("Extracted input to short");
            }
            let mut new_input_path = PathBuf::from(&input_dir);
            new_input_path.push(format!("input{:#04}", i));
            fs::write(new_input_path, new_input_image).unwrap();
        }
        return input_dir;
    }
}

