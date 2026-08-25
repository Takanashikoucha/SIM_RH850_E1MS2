//! Memory model for RH850 E1MS2: models the real address map with distinct
//! regions for code flash, local RAM, global RAM, and on-chip I/O.
//!
//! E1MS2 address map (HW manual §4.1):
//!   0x00000000–0x003FFFFF  Code flash (user)        4 MB   read-only
//!   0x01000000–0x01007FFF  User boot flash          32 KB  read-only
//!   0x10000000–0x1FFFFFFF  On-chip I/O (low)                 read/write
//!   0xFEBF0000–0xFEBFFFFF  Local RAM (main)       64 KB   read/write
//!   0xFEDF0000–0xFEDFFFFF  Local RAM (self)       64 KB   read/write
//!   0xFEEE0000–0xFEF1FFFF  Global RAM            256 KB   read/write
//!   0xFF000000–0xFFFDFFFF  On-chip I/O (high)                read/write
//!
//! The simulator models:
//!   - Flash as a ROM buffer mapped at a configurable base (default 0x0).
//!   - Local RAM as a contiguous buffer at its real base address.
//!   - Global RAM as a contiguous buffer at its real base address.
//!   - I/O as a sparse HashMap (only registered addresses respond).
//!   - Unmapped reads return 0; unmapped writes are dropped.

use crate::state::G3;
use std::collections::HashMap;

// --- E1MS2 address map constants ---
pub const FLASH_BASE: u32 = 0x0000_0000;
pub const FLASH_SIZE: u32 = 0x0040_0000; // 4 MB

pub const BOOT_FLASH_BASE: u32 = 0x0100_0000;
pub const BOOT_FLASH_SIZE: u32 = 0x0000_8000; // 32 KB

pub const IO_LOW_BASE: u32 = 0x1000_0000;
pub const IO_LOW_SIZE: u32 = 0x1000_0000; // 256 MB

pub const LOCAL_RAM_BASE: u32 = 0xFEBF_0000;
pub const LOCAL_RAM_SIZE: u32 = 0x0001_0000; // 64 KB

pub const LOCAL_RAM_SELF_BASE: u32 = 0xFEDF_0000;
pub const LOCAL_RAM_SELF_SIZE: u32 = 0x0001_0000; // 64 KB

pub const GLOBAL_RAM_BASE: u32 = 0xFEEE_0000;
pub const GLOBAL_RAM_SIZE: u32 = 0x0004_0000; // 256 KB

pub const IO_HIGH_BASE: u32 = 0xFF00_0000;
pub const IO_HIGH_SIZE: u32 = 0x00FE_0000; // ~16 MB

/// Backward-compatible default: the total addressable space for the RAM buffer.
/// This is the size of the local RAM buffer (64 KB) + global RAM (256 KB)
/// stored as separate buffers, but the legacy `ram_size` parameter in
/// `Memory::new` is kept for API compatibility and controls the local RAM size.
pub const RAM_SIZE_DEFAULT: u32 = LOCAL_RAM_SIZE;

#[derive(Clone)]
pub struct Memory {
    /// Code flash (ROM) data, mapped at `rom_base`.
    pub rom: Vec<u8>,
    /// Base address of the code flash region.
    pub rom_base: u32,
    /// Code buffer at low addresses (0x0–0x3FFFFF flash region).
    /// In the simulator this is writable (unlike real flash) so tests can
    /// place instructions. Read priority: rom > ram > local_ram > global_ram > io.
    /// Named `ram` for backward compatibility with existing test code.
    pub ram: Vec<u8>,
    /// Size of the code/ram buffer (the low-address region).
    pub ram_size: u32,
    /// Local RAM buffer (64 KB at 0xFEBF0000).
    pub local_ram: Vec<u8>,
    /// Global RAM buffer (256 KB at 0xFEEE0000).
    pub global_ram: Vec<u8>,
    /// Sparse I/O register map (addr → 32-bit value).
    pub io: HashMap<u32, u32>,
    /// Log of RAM writes as (addr, old_value) for undo support.
    pub write_log: Vec<(u32, u8)>,
}

impl Memory {
    /// Create a memory with default E1MS2 address map.
    /// `ram_size` parameter sets the code buffer size (backward compat with
    /// tests that pass e.g. 0x1000). The local/global RAM sizes are fixed.
    pub fn new(ram_size: u32) -> Self {
        Memory {
            rom: Vec::new(),
            rom_base: FLASH_BASE,
            ram: vec![0u8; ram_size as usize],
            ram_size,
            local_ram: vec![0u8; LOCAL_RAM_SIZE as usize],
            global_ram: vec![0u8; GLOBAL_RAM_SIZE as usize],
            io: HashMap::new(),
            write_log: Vec::new(),
        }
    }

    /// Is `addr` inside the loaded ROM (code flash) region?
    pub fn in_rom(&self, addr: u32) -> bool {
        !self.rom.is_empty()
            && addr >= self.rom_base
            && addr < self.rom_base + self.rom.len() as u32
    }

    /// Is `addr` in the code flash region (0x0–0x3FFFFF)?
    pub fn in_flash(&self, addr: u32) -> bool {
        addr >= FLASH_BASE && addr < FLASH_BASE + FLASH_SIZE
    }

    /// Is `addr` in the local RAM region (0xFEBF0000–0xFEBFFFFF)?
    pub fn in_local_ram(&self, addr: u32) -> bool {
        addr >= LOCAL_RAM_BASE && addr < LOCAL_RAM_BASE + LOCAL_RAM_SIZE
    }

    /// Is `addr` in the global RAM region (0xFEEE0000–0xFEF1FFFF)?
    pub fn in_global_ram(&self, addr: u32) -> bool {
        addr >= GLOBAL_RAM_BASE && addr < GLOBAL_RAM_BASE + GLOBAL_RAM_SIZE
    }

    /// Is `addr` in an I/O region (0x10000000–0x1FFFFFFF or 0xFF000000–0xFFFDFFFF)?
    pub fn in_io(&self, addr: u32) -> bool {
        (addr >= IO_LOW_BASE && addr < IO_LOW_BASE + IO_LOW_SIZE)
            || (addr >= IO_HIGH_BASE && addr < IO_HIGH_BASE + IO_HIGH_SIZE)
    }

    /// Is `addr` in any mapped, writable RAM region?
    pub fn in_writable_ram(&self, addr: u32) -> bool {
        self.in_local_ram(addr) || self.in_global_ram(addr)
    }

    /// Is `addr` in any mapped region (flash, RAM, or I/O)?
    pub fn in_mapped(&self, addr: u32) -> bool {
        self.in_flash(addr)
            || (addr >= BOOT_FLASH_BASE && addr < BOOT_FLASH_BASE + BOOT_FLASH_SIZE)
            || self.in_local_ram(addr)
            || self.in_global_ram(addr)
            || self.in_io(addr)
    }

    /// Load code flash data at the given base address (default 0x0).
    pub fn load_rom(&mut self, data: &[u8], base: u32) {
        self.rom = data.to_vec();
        self.rom_base = base;
    }

    /// Read a byte from any mapped address. Unmapped addresses return 0.
    /// Priority: rom > code > local_ram > global_ram > io > 0.
    pub fn read_byte(&self, addr: u32) -> u8 {
        // Code flash (loaded ROM)
        if addr >= self.rom_base && addr < self.rom_base + self.rom.len() as u32 {
            return self.rom[(addr - self.rom_base) as usize];
        }
        // Code buffer (low address space, for test instructions / flash simulation)
        if addr < self.ram_size {
            return self.ram[addr as usize];
        }
        // Local RAM
        if self.in_local_ram(addr) {
            return self.local_ram[(addr - LOCAL_RAM_BASE) as usize];
        }
        // Global RAM
        if self.in_global_ram(addr) {
            return self.global_ram[(addr - GLOBAL_RAM_BASE) as usize];
        }
        // I/O (sparse)
        self.io.get(&addr).map(|w| *w as u8).unwrap_or(0)
    }

    pub fn read_half(&self, addr: u32) -> u16 {
        let lo = self.read_byte(addr) as u16;
        let hi = self.read_byte(addr + 1) as u16;
        (hi << 8) | lo
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let mut w = 0u32;
        for i in 0..4u32 {
            w |= (self.read_byte(addr + i) as u32) << (8 * i);
        }
        w
    }

    pub fn read_double(&self, addr: u32) -> u64 {
        let lo = self.read_word(addr) as u64;
        let hi = self.read_word(addr + 4) as u64;
        (hi << 32) | lo
    }

    /// Write a byte to a mapped address. Writes to ROM or unmapped addresses are dropped.
    /// The code buffer is writable (simulator convenience for placing instructions).
    pub fn write_byte(&mut self, addr: u32, v: u8) {
        // Flash (loaded ROM) is read-only
        if self.in_rom(addr) {
            return;
        }
        // Code buffer (simulator convenience — real flash would be read-only)
        if addr < self.ram_size {
            self.ram[addr as usize] = v;
            return;
        }
        // Local RAM
        if self.in_local_ram(addr) {
            let idx = (addr - LOCAL_RAM_BASE) as usize;
            let old = self.local_ram[idx];
            self.write_log.push((addr, old));
            self.local_ram[idx] = v;
            return;
        }
        // Global RAM
        if self.in_global_ram(addr) {
            let idx = (addr - GLOBAL_RAM_BASE) as usize;
            let old = self.global_ram[idx];
            self.write_log.push((addr, old));
            self.global_ram[idx] = v;
            return;
        }
        // I/O (sparse)
        if let Some(w) = self.io.get_mut(&addr) {
            *w = (*w & 0xFFFFFF00) | (v as u32);
        }
        // Writes to unmapped addresses are dropped
    }

    pub fn write_half(&mut self, addr: u32, v: u16) {
        self.write_byte(addr, v as u8);
        self.write_byte(addr + 1, (v >> 8) as u8);
    }

    pub fn write_word(&mut self, addr: u32, v: u32) {
        for i in 0..4u32 {
            self.write_byte(addr + i, (v >> (8 * i)) as u8);
        }
    }

    pub fn write_double(&mut self, addr: u32, v: u64) {
        self.write_word(addr, v as u32);
        self.write_word(addr + 4, (v >> 32) as u32);
    }

    /// Drain the write log (addr, old_value) pairs since the last drain.
    pub fn take_write_log(&mut self) -> Vec<(u32, u8)> {
        std::mem::take(&mut self.write_log)
    }

    /// Restore memory from a drained write log (undo).
    pub fn restore_from_log(&mut self, log: &[(u32, u8)]) {
        for &(addr, old) in log.iter().rev() {
            if self.in_local_ram(addr) {
                self.local_ram[(addr - LOCAL_RAM_BASE) as usize] = old;
            } else if self.in_global_ram(addr) {
                self.global_ram[(addr - GLOBAL_RAM_BASE) as usize] = old;
            }
        }
    }

    /// Return the low-address code buffer (for display/debug purposes).
    pub fn ram_bytes(&self) -> &[u8] {
        &self.ram
    }

    /// Return mutable low-address code buffer.
    pub fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }

    /// Return the local RAM buffer (0xFEBF0000–0xFEBFFFFF).
    pub fn local_ram_bytes(&self) -> &[u8] {
        &self.local_ram
    }

    /// Return mutable local RAM buffer.
    pub fn local_ram_mut(&mut self) -> &mut [u8] {
        &mut self.local_ram
    }

    /// Return the global RAM buffer (0xFEEE0000–0xFEF1FFFF).
    pub fn global_ram_bytes(&self) -> &[u8] {
        &self.global_ram
    }

    /// Return mutable global RAM buffer.
    pub fn global_ram_mut(&mut self) -> &mut [u8] {
        &mut self.global_ram
    }
}

impl G3 {
    // Convenience: the G3 state itself does not own memory; memory is passed to exec.
}
