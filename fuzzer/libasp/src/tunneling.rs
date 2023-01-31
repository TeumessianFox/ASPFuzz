/// Tunneling allows statically or dynamically setting register values at a specific PC value
/// This especially allows skipping comparisons with magic values or checksums

use libafl_qemu::*;
use log;

static mut TUNNELS_CMPS: Vec<(GuestAddr, String)> = vec![];

pub fn add_tunnels_cmp(addr: GuestAddr, r0: &str, emu: &Emulator) {
    let cmp = (addr, r0.to_string());
    unsafe { TUNNELS_CMPS.push(cmp); }
    emu.set_hook(addr, tunnels_cmp_hook, emu as *const _ as u64, false);
}

extern "C" fn tunnels_cmp_hook(pc: GuestAddr, data: u64) {
    log::debug!("Tunnels cmp hook: pc={:#x}", pc);
    let emu = unsafe { (data as *const Emulator).as_ref().unwrap() };
    for cmp in unsafe { TUNNELS_CMPS.iter() } {
        if cmp.0 == pc {
            log::debug!("Found matching tunnels cmp: [{:#x}, {}]", cmp.0, cmp.1);
            if cmp.1.parse::<GuestAddr>().is_ok(){
                emu.write_reg(Regs::R0 as i32, cmp.1.parse::<u32>().unwrap()).unwrap();
                break;
            } else {
                let r0: u64 = emu.read_reg(str_reg_to_regs(&cmp.1)).unwrap();
                emu.write_reg(Regs::R0 as i32, r0).unwrap();
                break;
            }
        }
    }
}

pub fn str_reg_to_regs(reg: &str) -> Regs {
    match reg {
        "R0"    => return Regs::R0,
        "R1"    => return Regs::R1,
        "R2"    => return Regs::R2,
        "R3"    => return Regs::R3,
        "R4"    => return Regs::R4,
        "R5"    => return Regs::R5,
        "R6"    => return Regs::R6,
        "R7"    => return Regs::R7,
        "R8"    => return Regs::R8,
        "R9"    => return Regs::R9,
        "R10"   => return Regs::R10,
        "R11"   => return Regs::R11,
        "R12"   => return Regs::R12,
        "R13"   => return Regs::R13,
        "R14"   => return Regs::R14,
        "R15"   => return Regs::R15,
        "R25"   => return Regs::R25,
        "Sp"    => return Regs::Sp,
        "SP"    => return Regs::Sp,
        "Lr"    => return Regs::Lr,
        "LR"    => return Regs::Lr,
        "Pc"    => return Regs::Pc,
        "PC"    => return Regs::Pc,
        "Sb"    => return Regs::Sb,
        "SB"    => return Regs::Sb,
        "Sl"    => return Regs::Sl,
        "SL"    => return Regs::Sl,
        "Fp"    => return Regs::Fp,
        "FP"    => return Regs::Fp,
        "Ip"    => return Regs::Ip,
        "IP"    => return Regs::Ip,
        "Cpsr"  => return Regs::Cpsr,
        "CPSR"  => return Regs::Cpsr,
        _       => panic!("Cannot match to valid ARM register"),
    }
}
