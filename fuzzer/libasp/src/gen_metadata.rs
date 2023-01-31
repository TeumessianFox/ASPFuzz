/// Generating metadata whenever a test-case is an objective
/// Saves all register values

use libafl_qemu::*;
use libafl::prelude::*;

use log;
use serde::{Deserialize, Serialize};

/// A custom testcase metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct CustomMetadata {
    pub r0: String,
    pub r1: String,
    pub r2: String,
    pub r3: String,
    pub r4: String,
    pub r5: String,
    pub r6: String,
    pub r7: String,
    pub r8: String,
    pub r9: String,
    pub r10: String,
    pub r11: String,
    pub r12: String,
    pub sp: String,
    pub pc: String,
    pub lr: String,
    pub cpsr: String,
}

impl_serdeany!(CustomMetadata);

impl CustomMetadata {
    /// Creates a new [`struct@CustomMetadata`]
    #[must_use]
    pub fn new(regs: Vec<u64>) -> Self {
        Self {
            r0: format!("{:#010x}", regs[0]),
            r1: format!("{:#010x}", regs[1]),
            r2: format!("{:#010x}", regs[2]),
            r3: format!("{:#010x}", regs[3]),
            r4: format!("{:#010x}", regs[4]),
            r5: format!("{:#010x}", regs[5]),
            r6: format!("{:#010x}", regs[6]),
            r7: format!("{:#010x}", regs[7]),
            r8: format!("{:#010x}", regs[8]),
            r9: format!("{:#010x}", regs[9]),
            r10: format!("{:#010x}", regs[10]),
            r11: format!("{:#010x}", regs[11]),
            r12: format!("{:#010x}", regs[12]),
            sp: format!("{:#010x}", regs[13]),
            pc: format!("{:#010x}", regs[14]),
            lr: format!("{:#010x}", regs[15]),
            cpsr: format!("{:#010x}", regs[16]),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomMetadataFeedback {
    emulator: u64,
}

impl<S> Feedback<S> for CustomMetadataFeedback
where
    S: UsesInput  + HasClientPerfMonitor,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer,
        OT: ObserversTuple<S>,
    {
        log::info!("CustomMetadataFeedback=True");
        Ok(true)
    }

    fn append_metadata(&mut self, _state: &mut S, testcase: &mut Testcase<S::Input>) -> Result<(), Error> {
        let emu = unsafe { (self.emulator as *const Emulator).as_ref().unwrap() };
        // Read regs
        let mut regs = Vec::new();
        for r in Regs::iter() {
           regs.push(emu.read_reg(r).unwrap());
        }
        testcase.add_metadata(CustomMetadata::new(regs));
        Ok(())
    }
}

impl Named for CustomMetadataFeedback {
    #[inline]
    fn name(&self) -> &str {
        "CustomMetadataFeedback"
    }
}

impl CustomMetadataFeedback {
    /// Creates a new [`CustomMetadataFeedback`]
    #[must_use]
    pub fn new(emulator: u64) -> Self {
        Self {
            emulator
        }
    }
}
