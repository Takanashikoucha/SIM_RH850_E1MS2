//! Exception and interrupt acknowledgment for RH850G3MH.
//! Implements the direct-vector method (Table 4.3) and basic PSW updates per Table 4.1.

use crate::state::*;

/// Whether the cause is handled at EI level (saved to EIPC/EIPSW, EIIC) vs FE level.
fn is_ei_level(cause: u16) -> bool {
    let b = cause & 0xFF;
    if cause >= 0x1000 && cause < 0x1200 {
        return true;
    }
    matches!(b, 0x71 | 0x40 | 0x50) // FPINT, TRAP0, TRAP1
        || (b >= 0x40 && b < 0x60) // TRAP vectors 0x40-0x5F
        || (cause >= 0x8000 && cause < 0x8100) // SYSCALL
}

/// Acknowledge an exception. `cause` is the full cause code; `fault_pc` is the PC of the
/// faulting instruction.
pub fn acknowledge(g: &mut G3, cause: u16, fault_pc: u32) {
    let b = cause & 0xFF;

    // Reset: PC <- RBASE, PSW <- reset value
    if b == CAUSE_RESET {
        g.pc = g.rb & !1;
        g.psw = PSW_RESET_VAL;
        g.stopped = true;
        g.stop_reason = "reset acknowledged".into();
        return;
    }

    let ei = is_ei_level(cause);

    // Base register: EBASE if PSW.EBV==1 else RBASE.
    let base = if g.psw & PSW_EBV != 0 {
        g.ebase
    } else {
        g.rb
    };
    let handler = base + vector_offset(cause);

    if ei {
        g.eipc = fault_pc;
        g.eipsw = g.psw;
        g.eiic = cause as u32;
    } else {
        g.fepc = fault_pc;
        g.fepsw = g.psw;
        g.feic = cause as u32;
    }

    // PSW updates per Table 4.1: UM <- 0 for exceptions.
    g.psw &= !PSW_UM;
    // EBV: FE-level exceptions set EBV=1; RIE clears it; EIINT/traps retain.
    if !ei {
        g.psw |= PSW_EBV;
    }
    if b == CAUSE_RIE {
        g.psw &= !PSW_EBV;
    }

    g.pc = handler & !1;
}
