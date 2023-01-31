/// Custom resetting of the state aka. snapshotting

use std::fmt::{
    Debug,
    Formatter,
};
use libafl_qemu::*;
use std::io::Write;
use std::fs::File;
use std::str::FromStr;
use log;

const SRAM_START : GuestAddr        = 0x0;
const LAZY_SRAM_SIZE : GuestAddr    = 0x1300;

pub struct ResetState {
    saved :                 bool,
    sram_size:              GuestAddr,
    num_loads :             usize,
    regs :                  Vec<u64>,
    sram :                  Vec<u8>,
    timer_count_0 :         u64,
    timer_count_1 :         u64,
    timer_control_0 :       u64,
    timer_control_1 :       u64,
    smn_slots :             [u32; 32],
}

#[derive(Default)]
pub enum ResetLevel {
    /*
     *  Loading snapshot:
     *  - R0-R15, CPSR
     */
    SuperLazy,

    /*
     *  Loading snapshot:
     *  - R0-R15, CPSR
     *  - SRAM slice
     *      - Zen1 & Zen+ [0x3ED00, 0x40000]
     *      - Zen2 & Zen3 [0x4ED00, 0x50000]
     */
    #[default]
    Lazy,

    /*
     *  Loading snapshot:
     *  - R0-R15, CPSR
     *  - SRAM
     *      - Zen1 & Zen+ [0x0, 0x40000]
     *      - Zen2 & Zen3 [0x0, 0x50000]
     *  - Timer
     *  - SMN Control
     */
    RustSnapshot,

    /*
     * WIP
     */
    QemuSnapshot,

    /*
     *  Resetting:
     *  - CPU
     *  - SRAM
     *      - Zen1 & Zen+ [0x0, 0x40000]
     *      - Zen2 & Zen3 [0x0, 0x50000]
     *  - Timer
     *  - SMN Control
     *  Executing until until harness entry
     */
    HardReset,
}

extern "C" {
    static mut aspfuzz_timer_count_0: u64;
    static mut aspfuzz_timer_control_0: u64;
    static mut aspfuzz_timer_count_1: u64;
    static mut aspfuzz_timer_control_1: u64;
    static mut aspfuzz_smn_slots: [u32; 32];
    fn aspfuzz_smn_update_slot(idx: u32);
}

impl ResetState {
    pub fn new(sram_size: GuestAddr) -> Self {
        Self {
            saved :             false,
            sram_size:          sram_size,
            num_loads :         0,
            regs :              vec![],
            sram :              vec![0; sram_size.try_into().unwrap()],
            timer_count_0 :     0,
            timer_count_1 :     0,
            timer_control_0 :   0,
            timer_control_1 :   0,
            smn_slots :         [0; 32],
        }
    }

    fn save_full(&mut self, emu: &Emulator) {
        log::info!("Saving full snapshot");

        // Saving registers
        for r in Regs::iter() {
            self.regs.push(emu.read_reg(r).unwrap());
        }

        // Saving SRAM
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        unsafe {
            cpu.read_mem(SRAM_START, &mut self.sram);
        }

        // Saving ASP timer state
        unsafe {
            self.timer_count_0 = aspfuzz_timer_count_0;
            self.timer_count_1 = aspfuzz_timer_count_1;
            self.timer_control_0 = aspfuzz_timer_control_0;
            self.timer_control_1 = aspfuzz_timer_control_1;
        }

        // Saving SMN slot controller state
        unsafe {
            self.smn_slots = aspfuzz_smn_slots;
        }
    }

    /* Super lazy reset */
    fn load_super_lazy(&self, emu: &Emulator) {
        // Resetting registers
        for (r, v) in self.regs.iter().enumerate() {
            emu.write_reg(r as i32, *v).unwrap();
        }
    }

    /* Lazy snapshot reset */
    fn load_lazy(&self, emu: &Emulator) {
        log::info!("Loading lazy");

        // Resetting registers
        self.load_super_lazy(emu);

        // Resetting SRAM (predefined section)
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        let sram_slice = &self.sram[((self.sram_size-LAZY_SRAM_SIZE) as usize)..(self.sram_size as usize)];
        unsafe {
            cpu.write_mem(self.sram_size-LAZY_SRAM_SIZE, &sram_slice);
        }
    }

    /* Rust snapshot reset */
    fn load_rust_snapshot(&self, emu: &Emulator) {
        log::info!("Loading Rust snapshot");

        // Resetting registers
        self.load_super_lazy(emu);

        // Resetting SRAM
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        unsafe {
            cpu.write_mem(SRAM_START, &self.sram);
        }

        // Resetting timer
        unsafe {
            if aspfuzz_timer_count_0 != self.timer_count_0 {
                log::debug!("Timer count 0: resetting to {:#x}", self.timer_count_0);
                aspfuzz_timer_count_0 = self.timer_count_0;
            }
            if aspfuzz_timer_count_1 != self.timer_count_1 {
                log::debug!("Timer count 1: resetting to {:#x}", self.timer_count_1);
                aspfuzz_timer_count_1 = self.timer_count_1;
            }
            if aspfuzz_timer_control_0 != self.timer_control_0 {
                log::debug!("Timer control 0: resetting to {:#x}", self.timer_control_0);
                aspfuzz_timer_control_0 = self.timer_control_0;
            }
            if aspfuzz_timer_control_1 != self.timer_control_1 {
                log::debug!("Timer control 1: resetting to {:#x}", self.timer_control_1);
                aspfuzz_timer_control_1 = self.timer_control_1;
            }
        }

        // Resetting SMN slot controller
        let current_smn_slots;
        unsafe {
            current_smn_slots =  aspfuzz_smn_slots;
            aspfuzz_smn_slots = self.smn_slots;
        }
        for (i, (snapshot, current)) in current_smn_slots.iter().zip(self.smn_slots.iter()).enumerate() {
            if snapshot != current {
                log::debug!("SMN slot {i} not correct anymore:");
                log::debug!("\tsnapshot: {snapshot:#x}");
                log::debug!("\tcurrent : {current:#x}");
                unsafe { aspfuzz_smn_update_slot(i as u32) }
            }
        }
    }

    /* Qemu snapshot reset */
    fn load_qemu_snapshot(&self, _emu: &Emulator) {
        panic!("QEMU snapshot unimplemented!");
    }

    /* Hard reset */
    fn load_hard_reset(&self, emu: &Emulator) {
        log::info!("Loading hard snapshot");

        // Resetting CPU
        log::debug!("Starting CPU reset");
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        cpu.cpu_reset();
        log::debug!("CPU reset successful");

        // Zero SRAM
        let zero_sram = vec![0; self.sram_size.try_into().unwrap()];
        unsafe {
            cpu.write_mem(SRAM_START, &zero_sram);
        }

        // Zero timer
        unsafe {
            aspfuzz_timer_count_0 = 0;
            aspfuzz_timer_count_1 = 0;
            aspfuzz_timer_control_0 = 0;
            aspfuzz_timer_control_1 = 0;
        }

        // Zero SMN slots
        unsafe {
            aspfuzz_smn_slots = [0; 32];
            for (i,_) in aspfuzz_smn_slots.iter().enumerate() {
                aspfuzz_smn_update_slot(i as u32)
            }
        }

        // Run until fuzzing start address
        emu.set_breakpoint(self.regs[Regs::Pc as usize] as GuestAddr);
        emu.start(&cpu);
        emu.remove_breakpoint(self.regs[Regs::Pc as usize] as GuestAddr);
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        let pc: u64 = cpu.read_reg(Regs::Pc).unwrap();
        log::debug!("After CPU reset: PC={:#x}", pc);
    }

    pub fn sram_to_file(&self) {
        let mut file = File::create("sram.dump").unwrap();
        file.write(&self.sram).unwrap();
    }

    pub fn current_sram_to_file(&mut self, emu: &Emulator) {
        let cpu = emu.current_cpu().unwrap(); // ctx switch safe
        unsafe {
            cpu.write_mem(SRAM_START, &self.sram);
        }
        let mut file = File::create("sram.dump").unwrap();
        file.write(&self.sram).unwrap();
    }
}

pub trait Reset {
    fn save(&mut self, emu: &Emulator, level: &ResetLevel);
    fn load(&mut self, emu: &Emulator, level: &ResetLevel);
}

impl Reset for ResetState {
    fn save(&mut self, emu: &Emulator, level: &ResetLevel) {
        if self.saved {
            log::error!("State has already been saved!");
            return
        }
        match level {
            ResetLevel::SuperLazy => self.save_full(emu),
            ResetLevel::Lazy => self.save_full(emu),
            ResetLevel::RustSnapshot => self.save_full(emu),
            ResetLevel::QemuSnapshot => self.save_full(emu),
            ResetLevel::HardReset => self.save_full(emu),
        };
        self.saved = true;
    }

    fn load(&mut self, emu: &Emulator, level: &ResetLevel){
        match level {
            ResetLevel::SuperLazy => self.load_super_lazy(emu),
            ResetLevel::Lazy => self.load_lazy(emu),
            ResetLevel::RustSnapshot => self.load_rust_snapshot(emu),
            ResetLevel::QemuSnapshot => self.load_qemu_snapshot(emu),
            ResetLevel::HardReset => self.load_hard_reset(emu),
        };
        self.num_loads += 1;
    }
}

impl Debug for ResetLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(),std::fmt::Error> {
        let mut out_str = "".to_string();
        match *self {
            ResetLevel::SuperLazy => out_str.push_str(&"SuperLazy".to_string()),
            ResetLevel::Lazy => out_str.push_str(&"Lazy".to_string()),
            ResetLevel::RustSnapshot => out_str.push_str(&"RustSnapshot".to_string()),
            ResetLevel::QemuSnapshot => out_str.push_str(&"QemuSnapshot".to_string()),
            ResetLevel::HardReset => out_str.push_str(&"HardReset".to_string()),
        }
        write!(f, "{}", out_str)
    }
}

impl FromStr for ResetLevel {
    type Err = ();
    fn from_str(input: &str) -> Result<ResetLevel, ()> {
        match input {
            "SuperLazy"     => Ok(ResetLevel::SuperLazy),
            "Lazy"          => Ok(ResetLevel::Lazy),
            "RustSnapshot"  => Ok(ResetLevel::RustSnapshot),
            "QemuSnapshot"  => Ok(ResetLevel::QemuSnapshot),
            "HardReset"     => Ok(ResetLevel::HardReset),
            _               => Err(()),
        }
    }
}

impl Debug for ResetState {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut out_str = "".to_string();

        /* Stats to string */
        out_str.push_str(&format!("[{}]\n", if self.saved { "INIT" } else { "UNINIT" }));
        out_str.push_str(&"Stats:\n".to_string());
        out_str.push_str(&format!("\tLoads =\t{}\n", self.num_loads));

        /* Register to string */
        out_str.push_str(&"Regs:\n".to_string());
        for (i, item) in self.regs.iter().enumerate() {
            let mut reg_name: String = "UDef".to_string();
            if i < 13 {
                reg_name = format!("R{}", i+1);
            }else if i == 13 {
                reg_name = "Sp".to_string();
            }else if i == 14 {
                reg_name = "Lr".to_string();
            }else if i == 15 {
                reg_name = "Pc".to_string();
            }else if i == 16 {
                reg_name = "CPSR".to_string();
            }
            let item_str = format!("\t{} =\t{:#08X}\n", reg_name, *item as usize);
            out_str.push_str(&item_str);
        }

        /* SRAM status to string */
        out_str.push_str(&"SRAM:\n".to_string());
        out_str.push_str(&format!("\tNon zero =\t{}\n", self.sram.iter().filter(|&n| *n != 0).count()));
        let mut addr_first = 0;
        for (i, item) in self.sram.iter().enumerate() {
            if *item != 0 {
                addr_first = i;
                break
            }
        }
        out_str.push_str(&format!("\tAddr first =\t{:#08X}\n", addr_first));

        write!(f, "{}", out_str)
    }
}
