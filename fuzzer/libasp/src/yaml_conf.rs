/// Parsing the YAML config file

use libafl_qemu::*;
use crate::reset_state::ResetLevel;

use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::fmt::{Result, Formatter, Debug};

extern crate yaml_rust;
use yaml_rust::YamlLoader;

static mut CONF: YAMLConfig = YAMLConfig {
    config_file:                    String::new(),
    qemu_zen:                       String::new(),
    qemu_sram_size:                 0,
    qemu_on_chip_bl_path:           String::new(),
    flash_start_smn:                0,
    flash_size:                     0,
    flash_start_cpu:                0,
    flash_base:                     String::new(),
    input_initial:                  vec![],
    input_mem:                      vec![],
    input_fixed:                    vec![],
    input_total_size:               0,
    harness_start:                  0,
    harness_sinks:                  vec![],
    tunnels_cmps:                   vec![],
    crashes_breakpoints:            vec![],
    crashes_mmap_no_exec:           vec![],
    crashes_mmap_flash_read_fn:     0,
    crashes_mmap_no_write_flash_fn: vec![],
    crashes_mmap_no_write_hooks:    vec![],
    snapshot_default:               ResetLevel::Lazy,
    snapshot_on_crash:              ResetLevel::Lazy,
    snapshot_periodically:          ResetLevel::Lazy,
    snapshot_period:                0,
    init:                           false,
};

#[derive(Default)]
pub struct YAMLConfig {
    pub config_file:                    String,
    pub qemu_zen:                       String,
    pub qemu_sram_size:                 GuestAddr,
    pub qemu_on_chip_bl_path:           String,
    pub flash_start_smn:                GuestAddr,
    pub flash_size:                     usize,
    pub flash_start_cpu:                GuestAddr,
    pub flash_base:                     String,
    pub input_initial:                  Vec<String>,
    pub input_mem:                      Vec<(GuestAddr, usize)>,
    pub input_fixed:                    Vec<(GuestAddr, GuestAddr)>,
    pub input_total_size:               usize,
    pub harness_start:                  GuestAddr,
    pub harness_sinks:                  Vec<GuestAddr>,
    pub tunnels_cmps:                   Vec<(GuestAddr, String)>,
    pub crashes_breakpoints:            Vec<GuestAddr>,
    pub crashes_mmap_no_exec:           Vec<[GuestAddr; 2]>,
    pub crashes_mmap_flash_read_fn:     GuestAddr,
    pub crashes_mmap_no_write_flash_fn: Vec<(GuestAddr, GuestAddr, Vec<GuestAddr>)>,
    pub crashes_mmap_no_write_hooks:    Vec<(GuestAddr, GuestAddr, Vec<GuestAddr>)>,
    pub snapshot_default:               ResetLevel,
    pub snapshot_on_crash:              ResetLevel,
    pub snapshot_periodically:          ResetLevel,
    pub snapshot_period:                usize,
    init:                               bool,
}

pub fn init_global_conf(file: &str) {
    unsafe {
        CONF = YAMLConfig::new(file);
    }
}

pub fn borrow_global_conf() -> Option<&'static YAMLConfig> {
    let conf = unsafe { &CONF };
    if conf.init {
        return Some(conf);
    }else{
        None
    }
}

impl YAMLConfig {
    fn new(config_file: &str) -> Self {
        let mut file = File::options()
            .read(true)
            .write(false)
            .open(config_file)
            .expect("Unable to open yaml config file");
        let mut contents = String::new();

        file.read_to_string(&mut contents).expect("Unable to read yaml file");
        drop(file);

        let conf = &YamlLoader::load_from_str(&contents).unwrap()[0];

        let qemu_zen = conf["qemu"]["zen"].as_str().expect("Expecting 'qemu: zen:' in yaml");
        let qemu_sram_size;
        if qemu_zen == String::from("Zen1") {
            qemu_sram_size = 0x40000 as GuestAddr;
        } else if qemu_zen == String::from("Zen+") {
            qemu_sram_size = 0x40000 as GuestAddr;
        } else if qemu_zen == String::from("Zen2") {
            qemu_sram_size = 0x50000 as GuestAddr;
        } else if qemu_zen == String::from("Zen3") {
            qemu_sram_size = 0x50000 as GuestAddr;
        } else if qemu_zen == String::from("ZenTesla") {
            qemu_sram_size = 0x40000 as GuestAddr;
        } else {
            println!("{} generation not supported yet.", qemu_zen);
            std::process::exit(8);
        }
        let qemu_on_chip_bl_path = conf["qemu"]["on_chip_bl_path"].as_str().expect("Expecting 'qemu: on_chip_bl_path:' in yaml");

        let flash_start_smn = conf["flash"]["start_smn"].as_i64().expect("Expecting 'flash: start_smn:' in yaml") as GuestAddr;
        let flash_size = conf["flash"]["size"].as_i64().expect("Expecting 'flash: size:' in yaml") as usize;
        let flash_start_cpu = conf["flash"]["start_cpu"].as_i64().expect("Expecting 'flash: start_cpu:' in yaml") as GuestAddr;
        let flash_base = conf["flash"]["base"].as_str().expect("Expecting 'flash: base:' in yaml");

        let input_initial_iter = conf["input"]["initial"].as_vec().expect("Expecting 'input: initial:' in yaml").iter();
        let input_mem_iter = conf["input"]["mem"].as_vec().expect("Expecting 'input: mem:' in yaml").iter();
        let input_fixed_iter = conf["input"]["fixed"].as_vec().expect("Expecting 'input: fixed:' in yaml").iter();

        let harness_start = conf["harness"]["start"].as_i64().expect("Expecting 'harness: start:' in yaml") as GuestAddr;
        let harness_sinks_iter = conf["harness"]["sinks"].as_vec().expect("Expecting 'harness: sinks:' in yaml").iter();

        let tunnels_cmps_iter = conf["tunnels"]["cmps"].as_vec().expect("Expecting 'tunnels: cmps:' in yaml").iter();

        let crashes_breakpoints_iter = conf["crashes"]["breakpoints"].as_vec().expect("Expecting 'crashes: breakpoints:' in yaml").iter();
        let crashes_mmap_flash_read_fn = conf["crashes"]["mmap"]["flash_read_fn"].as_i64().expect("Expecting 'crashes: mmap: flash_read_fn:' in yaml") as GuestAddr;
        let crashes_mmap_no_exec_iter = conf["crashes"]["mmap"]["no_exec"].as_vec().expect("Expecting 'crashes: mmap: no_exec:' in yaml").iter();
        let crashes_mmap_no_write_flash_fn_iter = conf["crashes"]["mmap"]["no_write_flash_fn"].as_vec().expect("Expecting 'crashes: mmap: no_write:' in yaml").iter();
        let crashes_mmap_no_write_hooks_iter = conf["crashes"]["mmap"]["no_write_hooks"].as_vec().expect("Expecting 'crashes: mmap: no_write:' in yaml").iter();

        let mut input_initial = vec![];
        for initial in input_initial_iter {
            if initial.is_null() {
                break;
            }
            input_initial.push(initial.as_str().unwrap().to_string());
        }

        let mut input_mem = vec![];
        let mut input_total_size = 0;
        for mem in input_mem_iter {
            if mem["addr"].is_null() || mem["size"].is_null() {
                break;
            }
            input_mem.push((
                mem["addr"].as_i64().unwrap() as GuestAddr,
                mem["size"].as_i64().unwrap() as usize
            ));
            input_total_size += mem["size"].as_i64().unwrap() as usize;
        }

        let mut input_fixed = vec![];
        for fixed in input_fixed_iter {
            if fixed["addr"].is_null() || fixed["val"].is_null() {
                break;
            }
            input_fixed.push((
                fixed["addr"].as_i64().unwrap() as GuestAddr,
                fixed["val"].as_i64().unwrap() as GuestAddr
            ));
        }

        let mut harness_sinks = vec![];
        for sink in harness_sinks_iter {
            harness_sinks.push(sink.as_i64().expect("Expecting at least 1 sink") as GuestAddr);
        }

        let mut tunnels_cmps: Vec<(GuestAddr, String)> = vec![];
        for cmps in tunnels_cmps_iter {
            if cmps["addr"].is_null() || cmps["r0"].is_null() {
                break;
            }
            if cmps["r0"].as_str() != None {
                tunnels_cmps.push((
                    cmps["addr"].as_i64().unwrap() as GuestAddr,
                    cmps["r0"].as_str().unwrap().to_string()
                ));
            } else {
                tunnels_cmps.push((
                    cmps["addr"].as_i64().unwrap() as GuestAddr,
                    cmps["r0"].as_i64().unwrap().to_string()
                ));
            }
        }

        let mut crashes_breakpoints = vec![];
        for breakpoint in crashes_breakpoints_iter {
            if breakpoint.is_null() {
                break;
            }
            crashes_breakpoints.push(breakpoint.as_i64().unwrap() as GuestAddr);
        }

        let mut crashes_mmap_no_exec = vec![];
        for no_exec in crashes_mmap_no_exec_iter {
            if no_exec["begin"].is_null() || no_exec["end"].is_null() {
                break;
            }
            crashes_mmap_no_exec.push([
                no_exec["begin"].as_i64().unwrap() as GuestAddr,
                no_exec["end"].as_i64().unwrap() as GuestAddr
            ]);
        }

        let mut crashes_mmap_no_write_hooks = vec![];
        for no_write in crashes_mmap_no_write_hooks_iter {
            if no_write["begin"].is_null() || no_write["end"].is_null() {
                break;
            }
            let mut no_ldr_vec: Vec<GuestAddr> = vec![];
            if !no_write["no_ldr"].is_null() {
                let crashes_mmap_no_write_no_ldr_iter = no_write["no_ldr"].as_vec().expect("Expecting 'crashes: mmap: no_write_hooks: no_ldr:' in yaml").iter();
                for no_ldr in crashes_mmap_no_write_no_ldr_iter {
                    no_ldr_vec.push(no_ldr.as_i64().unwrap() as GuestAddr);
                }
            }
            crashes_mmap_no_write_hooks.push((
                no_write["begin"].as_i64().unwrap() as GuestAddr,
                no_write["end"].as_i64().unwrap() as GuestAddr,
                no_ldr_vec
            ));
        }

        let mut crashes_mmap_no_write_flash_fn = vec![];
        for no_write in crashes_mmap_no_write_flash_fn_iter {
            if no_write["begin"].is_null() || no_write["end"].is_null() {
                break;
            }
            let mut no_hook_vec: Vec<GuestAddr> = vec![];
            if !no_write["no_hook"].is_null() {
                let crashes_mmap_no_write_flash_fn_iter = no_write["no_hook"].as_vec().expect("Expecting 'crashes: mmap: no_write_flash_fn: no_hook:' in yaml").iter();
                for no_hook in crashes_mmap_no_write_flash_fn_iter {
                    no_hook_vec.push(no_hook.as_i64().unwrap() as GuestAddr);
                }
            }
            crashes_mmap_no_write_flash_fn.push((
                no_write["begin"].as_i64().unwrap() as GuestAddr,
                no_write["end"].as_i64().unwrap() as GuestAddr,
                no_hook_vec
            ));
        }

        Self {
            config_file:                    config_file.to_string(),
            qemu_zen:                       qemu_zen.to_string(),
            qemu_sram_size:                 qemu_sram_size,
            qemu_on_chip_bl_path:           qemu_on_chip_bl_path.to_string(),
            flash_start_smn:                flash_start_smn,
            flash_size:                     flash_size,
            flash_start_cpu:                flash_start_cpu,
            flash_base:                     flash_base.to_string(),
            input_initial:                  input_initial,
            input_mem:                      input_mem,
            input_fixed:                    input_fixed,
            input_total_size:               input_total_size,
            harness_start:                  harness_start,
            harness_sinks:                  harness_sinks,
            tunnels_cmps:                   tunnels_cmps,
            crashes_breakpoints:            crashes_breakpoints,
            crashes_mmap_no_exec:           crashes_mmap_no_exec,
            crashes_mmap_flash_read_fn:     crashes_mmap_flash_read_fn,
            crashes_mmap_no_write_flash_fn: crashes_mmap_no_write_flash_fn,
            crashes_mmap_no_write_hooks:    crashes_mmap_no_write_hooks,
            snapshot_default:               ResetLevel::from_str(conf["snapshot"]["default"].as_str().unwrap()).unwrap(),
            snapshot_on_crash:              ResetLevel::from_str(conf["snapshot"]["on_crash"].as_str().unwrap()).unwrap(),
            snapshot_periodically:          ResetLevel::from_str(conf["snapshot"]["periodically"].as_str().unwrap()).unwrap(),
            snapshot_period:                conf["snapshot"]["period"].as_i64().unwrap() as usize,
            init:                           true,
        }
    }
}

impl Debug for YAMLConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut out_str = "".to_string();
        out_str.push_str(&format!("#### YAML config: {} ####\n", self.config_file));
        out_str.push_str(&format!("Qemu:\n"));
        out_str.push_str(&format!("\tzen:\t\t\t\t{}\n", self.qemu_zen));
        out_str.push_str(&format!("\tsram size:\t\t\t{:#010x}\n", self.qemu_sram_size));
        out_str.push_str(&format!("\ton-chip bl path:\t\t{}\n", self.qemu_on_chip_bl_path));
        out_str.push_str(&format!("Flash:\n"));
        out_str.push_str(&format!("\tstart_smn:\t\t\t{:#010x}\n", self.flash_start_smn));
        out_str.push_str(&format!("\tsize:\t\t\t\t{:#010x}\n", self.flash_size));
        out_str.push_str(&format!("\tstart_cpu:\t\t\t{:#010x}\n", self.flash_start_cpu));
        out_str.push_str(&format!("\tbase:\t\t\t\t{}\n", self.flash_base));
        out_str.push_str(&format!("Input:\n"));
        out_str.push_str(&format!("\tinitial:\t\t\t{:?}\n", self.input_initial));
        out_str.push_str(&format!("\tmem:\t\t\t\t["));
        for mem in self.input_mem.iter() {
            out_str.push_str(&format!("({:#010x},{:#x}), ", mem.0, mem.1));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("\ttotal size:\t\t\t{:#x}\n", self.input_total_size));
        out_str.push_str(&format!("\tfixed:\t\t\t\t["));
        for fixed in self.input_fixed.iter() {
            out_str.push_str(&format!("({:#010x},{:#x}), ", fixed.0, fixed.1));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("Harness:\n"));
        out_str.push_str(&format!("\tstart:\t\t\t\t{:#010x}\n", self.harness_start));
        out_str.push_str(&format!("\tsinks:\t\t\t\t["));
        for sink in self.harness_sinks.iter() {
            out_str.push_str(&format!("{:#010x}, ", sink));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("Tunnels:\n"));
        out_str.push_str(&format!("\tcmps:\t\t\t\t["));
        for cmps in self.tunnels_cmps.iter() {
            out_str.push_str(&format!("({:#010x},R0={}), ", cmps.0, cmps.1));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("Crashes:\n"));
        out_str.push_str(&format!("\tbreakpoints:\t\t\t["));
        for breakpoint in self.crashes_breakpoints.iter() {
            out_str.push_str(&format!("{:#010x}, ", breakpoint));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("\tmmap no_exec:\t\t\t["));
        for no_exec in self.crashes_mmap_no_exec.iter() {
            out_str.push_str(&format!("({:#010x},{:#010x}), ", no_exec[0], no_exec[1]));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("\tmmap flash read function:\t{:#010x}\n", self.crashes_mmap_flash_read_fn));
        out_str.push_str(&format!("\tmmap no_write_flash_fn:\t\t["));
        for no_write in self.crashes_mmap_no_write_flash_fn.iter() {
            out_str.push_str(&format!("({:#010x},{:#010x}), [", no_write.0, no_write.1));
            for no_hook in &no_write.2 {
                out_str.push_str(&format!("{:#010x}, ", no_hook));
            }
            out_str.push_str(&format!("]), "));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("\tmmap no_write_hooks:\t\t["));
        for no_write in self.crashes_mmap_no_write_hooks.iter() {
            out_str.push_str(&format!("({:#010x},{:#010x}, [", no_write.0, no_write.1));
            for no_ldr in &no_write.2 {
                out_str.push_str(&format!("{:#010x}, ", no_ldr));
            }
            out_str.push_str(&format!("]), "));
        }
        out_str.push_str(&format!("]\n"));
        out_str.push_str(&format!("Snapshot:\n"));
        out_str.push_str(&format!("\tdefault:\t\t\t{:?}\n", self.snapshot_default));
        out_str.push_str(&format!("\ton crash:\t\t\t{:?}\n", self.snapshot_on_crash));
        out_str.push_str(&format!("\tperiodically:\t\t\t{:?}\n", self.snapshot_periodically));
        out_str.push_str(&format!("\tperiod:\t\t\t\t{}\n", self.snapshot_period));
        write!(f, "{}", out_str)
    }
}
