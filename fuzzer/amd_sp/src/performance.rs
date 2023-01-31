use libafl_qemu::*;
use libafl::prelude::*;
use libasp::*;

use std::fs;
use std::fs::File;
use std::io;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use std::time::Duration;
use std::process::exit;
use std::os::unix::io::AsRawFd;
use clap::Parser;

static mut ZEN_GENERATION: Option<String> = None;
static mut SRAM_SIZE: Option<GuestAddr> = None;
static mut ENTRY_POINT: Option<GuestAddr> = None;
static mut DIR_OFFSET: Option<GuestAddr> = None;
static mut PARSE_DIR: Option<GuestAddr> = None;
static mut COPY_PUBKEY: Option<GuestAddr> = None;
static mut VERIFY_PUBKEY: Option<GuestAddr> = None;
static mut LOAD_APP: Option<GuestAddr> = None;
static mut VERIFY_APP: Option<GuestAddr> = None;
static mut CALL_OFF_CHIP: Option<GuestAddr> = None;

#[allow(unused_mut)]
extern "C" fn on_vcpu(mut cpu: CPU) {
    let emu = cpu.emulator();

    // Creating output dirs
    let zen_generation = unsafe { ZEN_GENERATION.as_ref().unwrap() };
    let run_dir = PathBuf::from(format!("runs/perf/"));
    fs::create_dir_all(&run_dir).unwrap();
    let mut reset_state_dir = run_dir.clone();
    reset_state_dir.push(format!("{zen_generation}.stats"));
    let mut rs_file = File::create(reset_state_dir).unwrap();

    let sram_size: GuestAddr = unsafe { *SRAM_SIZE.as_ref().unwrap() };
    let mut rs = ResetState::new(sram_size);

    // Go to FUZZ_START
    let entry_point = unsafe { ENTRY_POINT.as_ref().unwrap() };
    emu.set_breakpoint(*entry_point);
    let start = Instant::now();
    emu.start(&cpu);
    let duration = start.elapsed();
    write!(rs_file, "Beginning to harness: \t{:>12}\n\n", format!("{:?}", duration)).unwrap();
    emu.remove_breakpoint(*entry_point);

    // Save emulator state
    rs.save(&emu, &ResetLevel::RustSnapshot);

    let num_iter = 10000;

    // Snapshot performance
    let reset_level = vec![
        ResetLevel::SuperLazy,
        ResetLevel::Lazy,
        ResetLevel::RustSnapshot,
        ResetLevel::HardReset
    ];
    write!(rs_file, "Averaged over {} iterations:\n", num_iter).unwrap();
    for lev in &reset_level {
        let start = Instant::now();
        for _ in 0..num_iter {
            rs.load(&emu, lev);
        }
        let duration = start.elapsed();
        write!(rs_file, "{:12}\t\t{:>12}/iter\n", format!("{:?}:", lev), format!("{:?}", duration/num_iter)).unwrap();
    }

    // Flash parsing runtime
    emu.set_breakpoint(0x100 as GuestAddr);
    let mut total_time: Duration = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    total_time = total_time/num_iter;
    write!(rs_file, "\nWhole flash parsing:\t\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*DIR_OFFSET.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*DIR_OFFSET.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "\nUntil dir offset function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*PARSE_DIR.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*PARSE_DIR.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until parse dir function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*COPY_PUBKEY.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*COPY_PUBKEY.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until copy pubkey function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*VERIFY_PUBKEY.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*VERIFY_PUBKEY.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until verify pubkey function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*LOAD_APP.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*LOAD_APP.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until load app function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*VERIFY_APP.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*VERIFY_APP.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until verify app function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    rs.load(&emu, &ResetLevel::HardReset);
    emu.set_breakpoint(unsafe {*CALL_OFF_CHIP.as_ref().unwrap()});
    total_time = Duration::from_secs(0);
    for _ in 0..num_iter {
        let start = Instant::now();
        emu.start(&cpu);
        let duration = start.elapsed();
        total_time += duration;
        rs.load(&emu, &ResetLevel::HardReset);
    }
    emu.remove_breakpoint(unsafe {*CALL_OFF_CHIP.as_ref().unwrap()});
    total_time = total_time/num_iter;
    write!(rs_file, "Until call off_chip function:\t\t{:>12}/iter\n", format!("{:?}", total_time)).unwrap();

    exit(0);
}

/// Performance measurement for the on-chip-bootloader from different AMD Zen generations.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)] // Read from Cargo.toml
struct Args {
   /// Run directory name
   #[arg(short, long)]
   zen_generation: Option<String>,

}

fn parse_args() -> Vec<String> {
    let cli_args = Args::parse();

    // Handle Zen generation
    if !vec![
            String::from("Zen1"),
            String::from("Zen+"),
            String::from("Zen2"),
            String::from("Zen3"),
            String::from("Zen4"),
            String::from("ZenTesla"),
        ].contains(&cli_args.zen_generation.as_ref().unwrap()){
        println!("{} not a valid Zen generation.", &cli_args.zen_generation.as_ref().unwrap());
        std::process::exit(2);
    }
    let zen_generation : &str;
    let on_chip_bl : &str;
    let uefi_image : &str;
    if cli_args.zen_generation.as_ref().unwrap() == &String::from("Zen1") {
        zen_generation = "amd-psp-zen";
        on_chip_bl = "bins/on-chip-bl-Ryzen-Zen1-Desktop";
        uefi_image = "bins/PRIME-X370-PRO-ASUS-3803.ROM";
        unsafe {
            SRAM_SIZE       = Some(0x4_0000);
            ENTRY_POINT     = Some(0xffff_4be4);
            DIR_OFFSET      = Some(0xffff_4af8);
            PARSE_DIR       = Some(0xffff_42d0);
            COPY_PUBKEY     = Some(0xffff_442c);
            VERIFY_PUBKEY   = Some(0xffff_44cc);
            LOAD_APP        = Some(0xffff_45f8);
            VERIFY_APP      = Some(0xffff_481c);
            CALL_OFF_CHIP   = Some(0xffff_48e4);
        }
    } else if cli_args.zen_generation.as_ref().unwrap() == &String::from("Zen+") {
        zen_generation = "amd-psp-zen+";
        on_chip_bl = "bins/on-chip-bl-Ryzen-Zen+-Desktop";
        uefi_image = "bins/PRIME-X370-PRO-ASUS-3803.ROM";
        unsafe {
            SRAM_SIZE       = Some(0x4_0000);
            ENTRY_POINT     = Some(0xffff_4b90);
            DIR_OFFSET      = Some(0xffff_4aa4);
            PARSE_DIR       = Some(0xffff_427c);
            COPY_PUBKEY     = Some(0xffff_43d8);
            VERIFY_PUBKEY   = Some(0xffff_4478);
            LOAD_APP        = Some(0xffff_45a4);
            VERIFY_APP      = Some(0xffff_47c8);
            CALL_OFF_CHIP   = Some(0xffff_4890);
        }
    } else if cli_args.zen_generation.as_ref().unwrap() == &String::from("Zen2") {
        zen_generation = "amd-psp-zen2";
        on_chip_bl = "bins/on-chip-bl-Ryzen-Zen2-Desktop";
        uefi_image = "bins/ASUS_PRIME-B450M-A-ASUS-1201.ROM";
        unsafe {
            SRAM_SIZE       = Some(0x5_0000);
            ENTRY_POINT     = Some(0xffff_2bf8);
            DIR_OFFSET      = Some(0xffff_27b0);
            PARSE_DIR       = Some(0xffff_1fc4);
            COPY_PUBKEY     = Some(0xffff_2140);
            VERIFY_PUBKEY   = Some(0xffff_21f4);
            LOAD_APP        = Some(0xffff_2908);
            VERIFY_APP      = Some(0xffff_23b0);
            CALL_OFF_CHIP   = Some(0xffff_24b8);
        }
    } else if cli_args.zen_generation.as_ref().unwrap() == &String::from("Zen3") {
        zen_generation = "amd-psp-zen3";
        on_chip_bl = "bins/on-chip-bl-Ryzen-Zen3-Desktop";
        uefi_image = "bins/ASUS_PRIME-B450M-A-ASUS-1201.ROM";
        unsafe {
            SRAM_SIZE       = Some(0x5_0000);
            ENTRY_POINT     = Some(0xffff_2bf8);
            DIR_OFFSET      = Some(0xffff_27b0);
            PARSE_DIR       = Some(0xffff_1fc4);
            COPY_PUBKEY     = Some(0xffff_2140);
            VERIFY_PUBKEY   = Some(0xffff_21f4);
            LOAD_APP        = Some(0xffff_2908);
            VERIFY_APP      = Some(0xffff_23b0);
            CALL_OFF_CHIP   = Some(0xffff_24b8);
        }
    } else if cli_args.zen_generation.as_ref().unwrap() == &String::from("ZenTesla") {
        zen_generation = "amd-psp-zentesla";
        on_chip_bl = "bins/on-chip-bl-Ryzen-ZenTesla";
        uefi_image = "bins/ZenTesla-BIOS-first-half.bin";
        unsafe {
            SRAM_SIZE       = Some(0x4_0000);
            ENTRY_POINT     = Some(0xffff_4650);
            DIR_OFFSET      = Some(0xffff_434c);
            PARSE_DIR       = Some(0xffff_3b7c);
            COPY_PUBKEY     = Some(0xffff_3cf0);
            VERIFY_PUBKEY   = Some(0xffff_3d90);
            LOAD_APP        = Some(0xffff_3ebc);
            VERIFY_APP      = Some(0xffff_410c);
            CALL_OFF_CHIP   = Some(0xffff_41d4);
        }
    } else {
        println!("{} generation not supported yet.", &cli_args.zen_generation.as_ref().unwrap());
        std::process::exit(3);
    }
    unsafe { ZEN_GENERATION = Some(cli_args.zen_generation.unwrap()); }

    let mut qemu_args: Vec<String> = vec![env::args().nth(0).unwrap()];
    qemu_args.extend(vec![
        "--machine".to_string(),
        zen_generation.to_string(),
        "--nographic".to_string(),
        "-device".to_string(),
        format!["loader,file={}/{},addr=0xffff0000,force-raw=on", env::var("PROJECT_DIR").unwrap(), on_chip_bl],
        "-global".to_string(),
        format!["driver=amd_psp.smnflash,property=flash_img,value={}/{}", env::var("PROJECT_DIR").unwrap(), uefi_image],
        "-bios".to_string(),
        format!["{}/{}", env::var("PROJECT_DIR").unwrap(), uefi_image],
    ]);

    return qemu_args;
}

pub fn fuzz() {
    env_logger::init();
    let env: Vec<(String, String)> = env::vars().collect();
    let qemu_args = parse_args();

    // Logs and prints to /dev/null
    let file_null = File::open("/dev/null").unwrap();
    let null_fd = file_null.as_raw_fd();
    dup2(null_fd, io::stdout().as_raw_fd()).unwrap();
    dup2(null_fd, io::stderr().as_raw_fd()).unwrap();

    // Start emulator
    let emu = Emulator::new(&qemu_args, &env);
    emu.set_vcpu_start(on_vcpu);
    unsafe {
        emu.run();
    }
}
