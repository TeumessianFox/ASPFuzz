use libafl::corpus::ondisk::OnDiskMetadataFormat;
use libafl::prelude::*;
use libafl_qemu::*;
use libafl_qemu::drcov::QemuDrCovHelper;

use libasp::*;

use rangemap::RangeMap;
use clap::Parser;

use log;
use chrono::Local;
use std::env;
use std::fs;
use std::path::{
    Path,
    PathBuf,
};
use std::time::Duration;
use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::exit;

#[cfg(not(feature = "multicore"))]
use nix::{self, unistd::dup};
#[cfg(not(feature = "multicore"))]
use std::fs::File;
#[cfg(not(feature = "multicore"))]
use std::io;
#[cfg(not(feature = "multicore"))]
use std::os::unix::io::AsRawFd;
#[cfg(not(feature = "multicore"))]
use std::os::unix::io::FromRawFd;

const ON_CHIP_ADDR: GuestAddr = 0xffff_0000;

static mut EMULATOR: u64 = 0;
static mut COUNTER_EDGE_HOOKS: usize = 0;
static mut COUNTER_WRITE_HOOKS: usize = 0;
static mut COUNTER_SNAPSHOT: usize = 0;
static mut CRASH_SNAPSHOT: bool = false;
static mut FLASH_READ_HOOK_ID: usize = 0;
static mut RUN_DIR_NAME: Option<String> = None;
#[cfg(feature = "multicore")]
static mut NUM_CORES: Option<u32> = None;

fn gen_block_hook<QT, S>(
    _hooks: &mut QemuHooks<QT, S>,
    _id: Option<&mut S>,
    src: GuestAddr,
) -> Option<u64>
where
    S: UsesInput,
    QT: QemuHelperTuple<S>,
{
    let conf = borrow_global_conf().unwrap();
    for no_exec in conf.crashes_mmap_no_exec.iter() {
        if src >= no_exec[0] && src < no_exec[1] {
            log::debug!("Generate block:");
            log::debug!("> src: {:#x}", src);
            unsafe { COUNTER_EDGE_HOOKS += 1 };
            log::debug!("> id: {:#x}", unsafe { COUNTER_EDGE_HOOKS });
            let emu = unsafe { (EMULATOR as *const Emulator).as_ref().unwrap() };
            emu.current_cpu().unwrap().trigger_breakpoint();
            return Some(unsafe { COUNTER_EDGE_HOOKS } as u64);
        }
    }
    if conf.crashes_mmap_no_write_flash_fn.len() != 0 {
        if conf.crashes_mmap_flash_read_fn == src {
            log::debug!("Adding block hook for flash_read_fn");
            unsafe {
                COUNTER_EDGE_HOOKS += 1;
                FLASH_READ_HOOK_ID = COUNTER_EDGE_HOOKS;
            }
            return Some(unsafe { COUNTER_EDGE_HOOKS } as u64);
        }
    }
    None
}

extern "C" fn exec_block_hook(id: u64, data: u64) {
    let emu = unsafe { (EMULATOR as *const Emulator).as_ref().unwrap() };
    if unsafe { FLASH_READ_HOOK_ID } == id as usize {
        let conf = borrow_global_conf().unwrap();
        let cpu = emu.current_cpu().unwrap();
        let pc: u64 = cpu.read_reg(Regs::Pc).unwrap();
        log::debug!("Flash read fn id was hit");
        if pc as GuestAddr == conf.crashes_mmap_flash_read_fn {
            let cpy_src: GuestAddr = cpu.read_reg::<libafl_qemu::Regs, u64>(Regs::R0).unwrap() as GuestAddr;
            let cpy_dest_start: GuestAddr = cpu.read_reg::<libafl_qemu::Regs, u64>(Regs::R1).unwrap() as GuestAddr;
            let cpy_len: GuestAddr = cpu.read_reg::<libafl_qemu::Regs, u64>(Regs::R2).unwrap() as GuestAddr;
            let cpy_dest_end: GuestAddr = cpy_dest_start + cpy_len;
            log::debug!("Flash read fn from {:#010x} to {:#010x} for {:#x} bytes", cpy_src, cpy_dest_start, cpy_len);
            for area in &conf.crashes_mmap_no_write_flash_fn {
                if (area.0 >= cpy_dest_start && area.0 < cpy_dest_end) ||
                    (area.1 >= cpy_dest_start && area.1 < cpy_dest_end) {
                    log::debug!("Flash read fn writes to [{:#010x}, {:#010x}]", area.0, area.1);
                    let cpy_lr: GuestAddr = cpu.read_reg::<libafl_qemu::Regs, u64>(Regs::Lr).unwrap() as GuestAddr;
                    log::debug!("Flash read fn called from {:#010x}", cpy_lr);
                    if !area.2.contains(&cpy_lr) {
                        log::info!("Flash read fn hook triggered!");
                        cpu.trigger_breakpoint();
                    }
                }
            }
        }
    } else {
        log::debug!("Execute block:");
        log::debug!("> id: {}", id);
        log::debug!("> data: {}", data);
        emu.current_cpu().unwrap().trigger_breakpoint();
    }
}

fn gen_writes_hook<QT, S>(
    _hooks: &mut QemuHooks<QT, S>,
    _id: Option<&mut S>,
    src: GuestAddr,
    size: usize,
) -> Option<u64>
where
    S: UsesInput,
    QT: QemuHelperTuple<S>,
{
    let conf = borrow_global_conf().unwrap();
    for no_write in conf.crashes_mmap_no_write_hooks.iter() {
        for no_ldr in &no_write.2 {
            if src == *no_ldr {
                log::debug!("Skipping generation hook for {:#010x}", src);
                return None;
            }
        }
    }
    log::debug!("Generate writes:");
    log::debug!("> src: {:#x}", src);
    log::debug!("> size: {}", size);
    unsafe { COUNTER_WRITE_HOOKS += 1 };
    log::debug!("> id: {:#x}", unsafe { COUNTER_WRITE_HOOKS });
    return Some(unsafe { COUNTER_WRITE_HOOKS } as u64);
}

extern "C" fn exec_writes_hook(id: u64, addr: GuestAddr, data: u64) {
    let conf = borrow_global_conf().unwrap();
    for no_write in conf.crashes_mmap_no_write_hooks.iter() {
        if addr >= no_write.0 && addr < no_write.1 {
            log::debug!("Execute writes:");
            log::debug!("> id: {:#x}", id);
            log::debug!("> addr: {:#x}", addr);
            log::debug!("> data: {}", data);
            let emu = unsafe { (EMULATOR as *const Emulator).as_ref().unwrap() };
            emu.current_cpu().unwrap().trigger_breakpoint();
        }
    }
}
extern "C" fn exec_writes_hook_n(id: u64, addr: GuestAddr, size: usize, data: u64) {
    let conf = borrow_global_conf().unwrap();
    for no_write in conf.crashes_mmap_no_write_hooks.iter() {
        if addr >= no_write.0 && addr < no_write.1 {
            log::debug!("Execute writes:");
            log::debug!("> id: {:#x}", id);
            log::debug!("> addr: {:#x}", addr);
            log::debug!("> size: {}", size);
            log::debug!("> data: {}", data);
            let emu = unsafe { (EMULATOR as *const Emulator).as_ref().unwrap() };
            emu.current_cpu().unwrap().trigger_breakpoint();
        }
    }
}

extern "C" {
    fn aspfuzz_write_smn_flash(
        addr: GuestAddr,
        len: i32,
        buf: *mut u8,
    );
}
pub unsafe fn write_flash_mem(addr: GuestAddr, buf: &[u8]) {
    aspfuzz_write_smn_flash(addr, buf.len() as i32, buf.as_ptr() as *mut u8);
}

#[cfg(feature = "debug")]
fn print_input(input: &[u8]) {
    let mut out_str = "input=[\n".to_string();
    let mut iter = input.iter();
    let mut counter = 0;
    let mut last_byte = false;
    let mut last_no_print = false;
    loop {
        let mut word: [u8; 4] = [0; 4];
        for i in 0..4 {
            let obyte = iter.next();
            match obyte {
                Some(byte) => word[i] = *byte,
                None => {
                    last_byte = true;
                    if i == 0 {
                        last_no_print = true;
                    }
                    break
                },
            }
        }
        if last_no_print {
            break;
        }
        unsafe {
            out_str.push_str(&format!(
                " {:08x},",
                std::mem::transmute::<[u8; 4], u32>(word)
            ));
        }
        counter += 1;
        if last_byte {
            break;
        }
        if counter % 4 == 0 {
            out_str.push_str(&format!("\n"));
        }
    }
    out_str.push_str(&format!("\n]"));

    log::info!("{}", out_str);
}

extern "C" fn on_vcpu(mut cpu: CPU) {
    let emu = cpu.emulator();
    let conf = borrow_global_conf().unwrap();

    // Create directory for this run
    let date = Local::now();
    let run_dir = if unsafe { RUN_DIR_NAME.as_ref().is_some() } {
        PathBuf::from(format!("runs/{}", unsafe { RUN_DIR_NAME.as_ref().unwrap() }))
    } else {
        PathBuf::from(format!("runs/{}", date.format("%Y-%m-%d_%H:%M")))
    };
    if !env::var("AFL_LAUNCHER_CLIENT".to_string()).is_ok() {
        if run_dir.exists() {
            fs::remove_dir_all(&run_dir).unwrap();
        }
    }
    fs::create_dir_all(&run_dir).unwrap();
    let mut input_dir = run_dir.clone();
    input_dir.push("inputs");
    fs::create_dir_all(&input_dir).unwrap();
    let mut log_dir = run_dir.clone();
    log_dir.push("logs");
    fs::create_dir_all(&log_dir).unwrap();
    let mut solutions_dir = run_dir.clone();
    solutions_dir.push("solutions");
    fs::create_dir_all(&solutions_dir).unwrap();
    let mut config_path = run_dir.clone();
    config_path.push("config.yaml");
    if !env::var("AFL_LAUNCHER_CLIENT".to_string()).is_ok() {
        fs::copy(&conf.config_file,&config_path).unwrap();
    }

    // Generate initial inputs
    let input_dir: PathBuf = InitialInput::new().create_initial_inputs(
        &conf.input_initial,
        &conf.input_mem,
        conf.flash_size as GuestAddr,
        conf.input_total_size,
        input_dir,
    );

    // Catching stdout/stderr
    #[cfg(not(feature = "multicore"))]
    let mut stdout_cpy = unsafe {
        let new_fd = dup(io::stdout().as_raw_fd()).unwrap();
        File::from_raw_fd(new_fd)
    };
    #[cfg(all(not(feature = "debug"), not(feature = "multicore")))]
    {
        let file_null = File::open("/dev/null").unwrap();
        let null_fd = file_null.as_raw_fd();
        dup2(null_fd, io::stdout().as_raw_fd()).unwrap();
        dup2(null_fd, io::stderr().as_raw_fd()).unwrap();
    }
    #[cfg(feature = "debug")]
    {
        let mut log_run_path = log_dir.clone();
        log_run_path.push("run.log");
        let runfile = File::create(log_run_path).unwrap();
        let run_fd = runfile.as_raw_fd();
        dup2(run_fd, io::stdout().as_raw_fd()).unwrap();
        dup2(run_fd, io::stderr().as_raw_fd()).unwrap();
    }

    // Configure ResetState and ExceptionHandler helpers
    let mut rs = ResetState::new(conf.qemu_sram_size);
    let eh = ExceptionHandler::new(ON_CHIP_ADDR);

    // Set fuzzing sinks
    for sink in &conf.harness_sinks {
        emu.set_breakpoint(*sink);
    }

    // Go to FUZZ_START
    emu.set_breakpoint(conf.harness_start);
    emu.start(&cpu);
    emu.remove_breakpoint(conf.harness_start);
    cpu = emu.current_cpu().unwrap(); // ctx switch safe
    let pc: u64 = cpu.read_reg(Regs::Pc).unwrap();
    log::debug!("#### First exit at {:#x} ####", pc);

    // Save emulator state
    rs.save(&emu, &ResetLevel::RustSnapshot);
    // Catching exceptions
    eh.start(&emu);
    // Setup tunnels cmps
    for cmp in &conf.tunnels_cmps {
        add_tunnels_cmp((*cmp).0, &(*cmp).1, &emu);
    }
    // Setup crash breakpoints
    for bp in &conf.crashes_breakpoints {
        emu.set_breakpoint(*bp);
    }

    // The closure that we want to fuzz
    let mut harness = |input: &BytesInput| {
        log::debug!("### Start harness");

        // Reset emulator state
        if unsafe { CRASH_SNAPSHOT } {
            unsafe { CRASH_SNAPSHOT = false; }
            rs.load(&emu, &conf.snapshot_on_crash);
        } else if unsafe { COUNTER_SNAPSHOT >= conf.snapshot_period } {
            unsafe { COUNTER_SNAPSHOT = 0; }
            rs.load(&emu, &conf.snapshot_periodically);
        } else {
            rs.load(&emu, &conf.snapshot_default);
        }

        #[cfg(feature = "debug")]
        print_input(input.bytes());

        // Input to memory
        let target = input.target_bytes();
        let mut target_buf = target.as_slice();
        if target_buf.len() > conf.input_total_size {
            target_buf = &target_buf[..conf.input_total_size];
        }
        let mut buffer = vec![0; conf.input_total_size];
        buffer[..target_buf.len()].copy_from_slice(target_buf);
        let mut buffer = buffer.as_slice();
        cpu = emu.current_cpu().unwrap(); // ctx switch safe
        for mem in conf.input_mem.iter() {
            unsafe { write_flash_mem(mem.0, &buffer[..mem.1]); }
            buffer = &buffer[mem.1..];
        }

        // Fixed values to memory
        for fixed in conf.input_fixed.iter() {
            let buffer = unsafe { std::mem::transmute::<u32, [u8; 4]>(fixed.1) };
            unsafe { write_flash_mem(fixed.0, &buffer); }
        }

        // Start the emulation
        let mut pc: u64 = cpu.read_reg(Regs::Pc).unwrap();
        log::debug!("Start at {:#x}", pc);
        emu.start(&cpu);

        // After the emulator finished
        pc = cpu.read_reg(Regs::Pc).unwrap();
        let r0: u64 = cpu.read_reg(Regs::R0).unwrap();
        log::debug!("End at {:#x} with R0={:#x}", pc, r0);
        unsafe { COUNTER_SNAPSHOT += 1; }
        // Look for crashes if no sink was hit
        if !conf.harness_sinks.iter().any(|&v| v == pc as GuestAddr) {
            // Don't trigger on exceptions
            if !(ON_CHIP_ADDR..(ON_CHIP_ADDR+4*ExceptionType::UNKNOWN as u32))
                .contains(&(pc as u32)) {
                unsafe {
                    COUNTER_SNAPSHOT = 0;
                    CRASH_SNAPSHOT = true;
                }
                log::info!("Found crash at {:#x}", pc);
                return ExitKind::Crash;
            }
        }
        log::debug!("End harness");
        ExitKind::Ok
    };

    #[allow(unused_mut)]
    let mut run_client = |state: Option<_>, mut mgr, _core_id| -> Result<(), Error> {
        // Create an observation channel using the coverage map
        let edges = unsafe { &mut edges::EDGES_MAP };
        let edges_counter = unsafe { &mut edges::MAX_EDGES_NUM };
        let edges_observer =
            HitcountsMapObserver::new(VariableMapObserver::new("edges", edges, edges_counter));

        // Feedback to rate the interestingness of an input
        let mut feedback = MaxMapFeedback::new(&edges_observer);

        #[allow(unused_assignments, unused_mut)]
        let mut objective_coverage_feedback =
                MaxMapFeedback::with_name("objective_coverage_feedback", &edges_observer);

        #[cfg(feature = "debug")]
        {
            objective_coverage_feedback =
                MaxMapFeedback::with_names_tracking("objective_coverage_feedback", "edges", true, false);
        }

        // A feedback to choose if an input is a solution or not
        let mut objective = feedback_and_fast!(
            feedback_and_fast!(
                feedback_or!(CrashFeedback::new(), ExceptionFeedback::new()),
                objective_coverage_feedback
            ),
            CustomMetadataFeedback::new( unsafe { EMULATOR } ) // always true, used to write metadata output whenever a test-case is a solution
        );

        // create a State from scratch
        let mut cloned_solutions_dir = solutions_dir.clone();
        let mut state = state.unwrap_or_else(|| {
            StdState::new(
                // RNG
                StdRand::with_seed(current_nanos()),
                // Corpus that will be evolved, we keep it in memory for performance
                InMemoryCorpus::new(),
                // Corpus in which we store solutions,
                // on disk so the user can get them after stopping the fuzzer
                OnDiskCorpus::new_save_meta(cloned_solutions_dir, Some(OnDiskMetadataFormat::JsonPretty)).unwrap(),
                // States of the feedbacks.
                // The feedbacks can report the data that should persist in the State.
                &mut feedback,
                // Same for objective feedbacks
                &mut objective,
            )
            .unwrap()
        });

        // Maximum input length
        state.set_max_size(conf.input_total_size);

        // A queue policy to get testcasess from the corpus
        let scheduler = QueueScheduler::new();

        // A fuzzer with feedbacks and a corpus scheduler
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

        // Configure DrCov helper
        let mut log_drcov_path = log_dir.clone();
        log_drcov_path.push("drcov.log");
        let mut rangemap = RangeMap::<usize, (u16, String)>::new();
        rangemap.insert(
            (0x0 as usize)..(0xffff_9000 as usize),
            (0, "on-chip-ryzen-zen.bl".to_string()),
        );

        // Configure QEMU hook helper
        let mut hooks = QemuHooks::new(&emu, tuple_list!(
                QemuEdgeCoverageHelper::new(QemuInstrumentationFilter::None),
                QemuDrCovHelper::new(
                    QemuInstrumentationFilter::None,
                    rangemap,
                    log_drcov_path,
                    false,
                )
        ));

        // Block hooks and write hooks for crash detection
        hooks.blocks_raw(Some(gen_block_hook), Some(exec_block_hook));
        if conf.crashes_mmap_no_write_hooks.len() != 0 {
            log::debug!("Adding write generation hooks");
            hooks.writes_raw(
                Some(gen_writes_hook),
                Some(exec_writes_hook),
                Some(exec_writes_hook),
                Some(exec_writes_hook),
                Some(exec_writes_hook),
                Some(exec_writes_hook_n)
            );
        } else {
            log::debug!("No write generation hooks");
        }

        let timeout = Duration::new(5, 0); // 5sec
        let mut executor = TimeoutExecutor::new(
            QemuExecutor::new(
                &mut hooks,
                &mut harness,
                tuple_list!(edges_observer),
                &mut fuzzer,
                &mut state,
                &mut mgr,
            )
            .unwrap(),
            timeout,
        );

        state
            .load_initial_inputs_forced(&mut fuzzer, &mut executor, &mut mgr, &[input_dir.clone()])
            .unwrap_or_else(|_| {
                println!("Failed to load initial corpus at {:?}", &input_dir);
                std::process::exit(0);
            });

        // Setup a mutational stage with a basic bytes mutator
        let mutator = StdScheduledMutator::new(havoc_mutations());
        let mut stages = tuple_list!(StdMutationalStage::new(mutator));

        log::info!("Starting fuzzing loop");
        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in the fuzzing loop");
        log::info!("Ending fuzzing loop");
        println!("END fuzzing loop");
        Ok(())
    };

    // Logging of LibAFL events
    let mut log_libafl_path = log_dir.clone();
    log_libafl_path.push("libafl.log");
    let logfile = PathBuf::from(log_libafl_path);
    let log = RefCell::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .open(&logfile)
            .unwrap(),
    );

    #[cfg(feature = "multicore")]
    {
        // The shared memory allocator
        let shmem_provider = StdShMemProvider::new().expect("Failed to init shared memory");

        // The stats reporter for the broker
        let monitor = MultiMonitor::new(|s| {
            println!("{s}");
            writeln!(log.borrow_mut(), "{s}").unwrap();
        });

        // Launcher config
        let broker_port = 1337;
        let num_cores = unsafe { NUM_CORES.as_ref().unwrap() } - 1;
        let cores = Cores::from_cmdline(&format!("0-{num_cores}")).unwrap();

        // Build and run a Launcher
        match Launcher::builder()
            .shmem_provider(shmem_provider)
            .broker_port(broker_port)
            .configuration(EventConfig::from_build_id())
            .monitor(monitor)
            .run_client(&mut run_client)
            .cores(&cores)
            .stdout_file(None)
            .build()
            .launch()
        {
            Ok(()) => (),
            Err(Error::ShuttingDown) => println!("Fuzzing stopped by user. Good bye."),
            Err(err) => panic!("Failed to run launcher: {:?}", err),
        }
    }

    #[cfg(not(feature = "multicore"))]
    {
        // The Monitor trait define how the fuzzer stats are displayed to the user
        let mon = SimpleMonitor::new(|s| {
            writeln!(&mut stdout_cpy, "{s}").unwrap();
            writeln!(log.borrow_mut(), "{s}").unwrap();
        });

        // The event manager handle the various events generated during the fuzzing loop
        // such as the notification of the addition of a new item to the corpus
        let mgr = SimpleEventManager::new(mon);

        run_client(None, mgr, 1).expect("Client closure failed");
    }
}

/// Fuzzing the on-chip-bootloader from different AMD Zen generations.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)] // Read from Cargo.toml
struct Args {
   /// YAML config file path
   #[arg(short, long)]
   yaml_path: String,

   /// Run directory name
   #[arg(short, long)]
   run_dir_name: Option<String>,

   /// Number of cores
   #[arg(short, long)]
   num_cores: Option<u32>,
}

fn parse_args() -> Vec<String> {
    let cli_args = Args::parse();

    // Parse YAML config
    if !Path::new(&cli_args.yaml_path).exists() {
        println!("YAML file path does not exist: {}", cli_args.yaml_path);
        exit(2);
    }
    init_global_conf(&cli_args.yaml_path);
    let conf = borrow_global_conf().unwrap();
    #[cfg(not(feature = "multicore"))]
    println!("{:?}", conf);

    // For multicore fuzzing a core number must be provided
    #[cfg(feature = "multicore")]
    if cli_args.num_cores.is_some() {
        unsafe { NUM_CORES = Some(cli_args.num_cores.unwrap()); }
    } else {
        println!("For multicore fuzzing a core number must be provided (`cargo make run_fast -h`)");
        exit(3);
    }

    //Check if pathes exist
    if !Path::new(&conf.qemu_on_chip_bl_path).exists() {
        println!("On-chip-bl file path does not exist: {}", &conf.qemu_on_chip_bl_path);
        exit(4);
    }
    if !Path::new(&conf.flash_base).exists() {
        println!("UEFI file path does not exist: {}", &conf.flash_base);
        exit(5);
    }

    // Handle Zen generation
    if !vec![
            String::from("Zen1"),
            String::from("Zen+"),
            String::from("Zen2"),
            String::from("Zen3"),
            String::from("Zen4"),
            String::from("ZenTesla"),
        ].contains(&conf.qemu_zen){
        println!("{} not a valid Zen generation.", &conf.qemu_zen);
        std::process::exit(6);
    }
    let zen_generation : &str;
    if conf.qemu_zen == String::from("Zen1") {
        zen_generation = "amd-psp-zen";
    } else if conf.qemu_zen == String::from("Zen+") {
        zen_generation = "amd-psp-zen+";
    } else if conf.qemu_zen == String::from("Zen2") {
        zen_generation = "amd-psp-zen2";
    } else if conf.qemu_zen == String::from("Zen3") {
        zen_generation = "amd-psp-zen3";
    } else if conf.qemu_zen == String::from("ZenTesla") {
        zen_generation = "amd-psp-zentesla";
    } else {
        println!("{} generation not supported yet.", &conf.qemu_zen);
        std::process::exit(7);
    }

    // Use run directory if provided
    if cli_args.run_dir_name.is_some() {
        unsafe { RUN_DIR_NAME = Some(cli_args.run_dir_name.unwrap()); }
    }

    // Create arguments to start QEMU with
    let mut qemu_args: Vec<String> = vec![env::args().nth(0).unwrap()];
    #[cfg(feature = "multicore")]
    qemu_args.append(&mut vec![
                "-trace".to_string(),
                "file=/dev/null".to_string()
    ]);
    #[cfg(feature = "debug")]
    qemu_args.append(&mut vec![
                "-d".to_string(),
                "trace:ccp_*,trace:psp_*".to_string()
    ]);
    qemu_args.extend(vec![
        "--machine".to_string(),
        zen_generation.to_string(),
        "--nographic".to_string(),
        "-device".to_string(),
        format!["loader,file={}/{},addr=0xffff0000,force-raw=on", env::var("PROJECT_DIR").unwrap(), &conf.qemu_on_chip_bl_path],
        "-global".to_string(),
        format!["driver=amd_psp.smnflash,property=flash_img,value={}/{}", env::var("PROJECT_DIR").unwrap(), &conf.flash_base],
        "-bios".to_string(),
        format!["{}/{}", env::var("PROJECT_DIR").unwrap(), &conf.flash_base],
    ]);

    return qemu_args;
}

pub fn fuzz() {
    env_logger::init();
    let env: Vec<(String, String)> = env::vars().collect();

    // Generate QEMU start arguments
    let qemu_args = parse_args();

    // Setup QEMU
    let emu = Emulator::new(&qemu_args, &env);
    unsafe {
        EMULATOR = &emu as *const _ as u64;
    }

    // Overwrite the QEMU vcpu loop with the fuzzer
    emu.set_vcpu_start(on_vcpu);

    // Start QEMU
    unsafe {
        emu.run();
    }
}
