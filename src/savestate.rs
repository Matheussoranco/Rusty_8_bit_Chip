use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

use crate::cpu::Cpu;
use crate::memory::Memory;
use crate::display::{Display, DISPLAY_WIDTH, DISPLAY_HEIGHT};
use crate::timers::Timers;

#[derive(Serialize, Deserialize)]
pub struct SaveState {
    pub v: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub waiting_for_key: Option<u8>,
    pub ram: Vec<u8>,
    pub pixels: Vec<bool>,
    pub delay: u8,
    pub sound: u8,
}

impl SaveState {
    pub fn capture(cpu: &Cpu, memory: &Memory, display: &Display, timers: &Timers) -> Self {
        SaveState {
            v: cpu.v,
            i: cpu.i,
            pc: cpu.pc,
            sp: cpu.sp,
            stack: cpu.stack,
            waiting_for_key: cpu.waiting_for_key,
            ram: memory.ram.to_vec(),
            pixels: display.pixels.to_vec(),
            delay: timers.delay,
            sound: timers.sound,
        }
    }

    pub fn restore(&self, cpu: &mut Cpu, memory: &mut Memory, display: &mut Display, timers: &mut Timers) {
        cpu.v = self.v;
        cpu.i = self.i;
        cpu.pc = self.pc;
        cpu.sp = self.sp;
        cpu.stack = self.stack;
        cpu.waiting_for_key = self.waiting_for_key;

        memory.ram.copy_from_slice(&self.ram);

        let len = DISPLAY_WIDTH * DISPLAY_HEIGHT;
        display.pixels.copy_from_slice(&self.pixels[..len]);
        display.dirty = true;

        timers.delay = self.delay;
        timers.sound = self.sound;
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let bytes = bincode::serialize(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        fs::write(path, bytes)
    }

    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let bytes = fs::read(path)?;
        bincode::deserialize(&bytes).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })
    }
}
