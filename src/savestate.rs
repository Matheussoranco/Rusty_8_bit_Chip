use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

use crate::cpu::Cpu;
use crate::memory::Memory;
use crate::display::{Display, DISPLAY_WIDTH, DISPLAY_HEIGHT};
use crate::timers::Timers;

#[derive(Serialize, Deserialize)]
pub struct SaveState {
    pub version: u32,
    pub v: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub waiting_for_key: Option<u8>,
    pub ram: [u8; 4096],
    pub pixels: [bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
    pub delay: u8,
    pub sound: u8,
    pub checksum: u64,
}

pub const SAVE_FORMAT_VERSION: u32 = 1;

/// Tamanho máximo aceito ao carregar um savestate (64 KiB; o estado
/// serializado tem ~6,2 KiB). Barrar arquivos gigantes antes de ler evita
/// que um `savestate.bin` trocado por engano (ou malicioso) estoure a
/// memória com `fs::read` irrestrito.
pub const MAX_SAVESTATE_BYTES: u64 = 64 * 1024;

fn checksum(
    version: u32,
    v: &[u8; 16],
    i: u16,
    pc: u16,
    sp: u8,
    stack: &[u16; 16],
    waiting_for_key: &Option<u8>,
    ram: &[u8; 4096],
    pixels: &[bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
    delay: u8,
    sound: u8,
) -> u64 {
    // FNV-1a 64 over all fields except `checksum` itself.
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    for b in version.to_le_bytes() {
        mix(b);
    }
    for b in v {
        mix(*b);
    }
    for b in i.to_le_bytes() {
        mix(b);
    }
    for b in pc.to_le_bytes() {
        mix(b);
    }
    mix(sp);
    for s in stack {
        for b in s.to_le_bytes() {
            mix(b);
        }
    }
    mix(waiting_for_key.unwrap_or(0xFF));
    for b in ram.iter() {
        mix(*b);
    }
    for p in pixels.iter() {
        mix(*p as u8);
    }
    mix(delay);
    mix(sound);
    h
}

impl SaveState {
    pub fn capture(cpu: &Cpu, memory: &Memory, display: &Display, timers: &Timers) -> Self {
        let version = SAVE_FORMAT_VERSION;
        let checksum = checksum(
            version,
            &cpu.v,
            cpu.i,
            cpu.pc,
            cpu.sp,
            &cpu.stack,
            &cpu.waiting_for_key,
            &memory.ram,
            &display.pixels,
            timers.delay,
            timers.sound,
        );
        SaveState {
            version,
            v: cpu.v,
            i: cpu.i,
            pc: cpu.pc,
            sp: cpu.sp,
            stack: cpu.stack,
            waiting_for_key: cpu.waiting_for_key,
            ram: memory.ram,
            pixels: display.pixels,
            delay: timers.delay,
            sound: timers.sound,
            checksum,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != SAVE_FORMAT_VERSION {
            return Err(format!(
                "unsupported savestate version {} (expected {})",
                self.version, SAVE_FORMAT_VERSION
            ));
        }
        // Fixed-size arrays are length-checked by construction (serde rejects
        // wrong lengths), but verify semantic ranges too.
        if self.sp as usize > self.stack.len() {
            return Err(format!("corrupt savestate: sp={} out of range", self.sp));
        }
        if self.pc as usize >= 4096 {
            return Err(format!("corrupt savestate: pc={:#06X} out of range", self.pc));
        }
        if self.i as usize >= 4096 {
            return Err(format!("corrupt savestate: I={:#06X} out of range", self.i));
        }
        if let Some(k) = self.waiting_for_key {
            if k > 0xF {
                return Err(format!("corrupt savestate: waiting_for_key={} out of range", k));
            }
        }
        let expected = checksum(
            self.version,
            &self.v,
            self.i,
            self.pc,
            self.sp,
            &self.stack,
            &self.waiting_for_key,
            &self.ram,
            &self.pixels,
            self.delay,
            self.sound,
        );
        if expected != self.checksum {
            return Err(format!(
                "corrupt savestate: checksum mismatch (expected {:016X}, got {:016X})",
                expected, self.checksum
            ));
        }
        Ok(())
    }

    pub fn restore(
        &self,
        cpu: &mut Cpu,
        memory: &mut Memory,
        display: &mut Display,
        timers: &mut Timers,
    ) -> Result<(), String> {
        self.validate()?;
        cpu.v = self.v;
        cpu.i = self.i;
        cpu.pc = self.pc;
        cpu.sp = self.sp;
        cpu.stack = self.stack;
        cpu.waiting_for_key = self.waiting_for_key;
        cpu.halted = false;
        cpu.halt_error = None;

        memory.ram = self.ram;

        display.pixels = self.pixels;
        display.dirty = true;

        timers.delay = self.delay;
        timers.sound = self.sound;
        Ok(())
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let bytes = bincode::serialize(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        // Escrita atômica: grava num temporário no mesmo diretório e
        // renomeia por cima. Se o processo morrer no meio, o savestate
        // anterior continua intacto em vez de ficar truncado.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, path)
    }

    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let len = fs::metadata(path)?.len();
        if len > MAX_SAVESTATE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "savestate too large ({} bytes, max {}): refusing to load",
                    len, MAX_SAVESTATE_BYTES
                ),
            ));
        }
        let bytes = fs::read(path)?;
        let state: Self = bincode::deserialize(&bytes).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        state.validate().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        Ok(state)
    }
}
