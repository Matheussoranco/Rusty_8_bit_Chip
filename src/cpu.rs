use crate::memory::Memory;
use crate::display::Display;
use crate::input::Input;
use crate::timers::Timers;
use rand::Rng;

pub struct Cpu {
    pub v: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    /// Some(reg) when waiting for a keypress; halts execution until a key is pressed.
    pub waiting_for_key: Option<u8>,
    /// Set when a fatal error halts the CPU (OOB memory, stack fault, bad opcode...).
    pub halted: bool,
    pub halt_error: Option<String>,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            v: [0; 16],
            i: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16],
            waiting_for_key: None,
            halted: false,
            halt_error: None,
        }
    }

    fn halt(&mut self, msg: String) -> String {
        self.halted = true;
        self.halt_error = Some(msg.clone());
        eprintln!("CPU halted: {}", msg);
        msg
    }

    pub fn execute_cycle(
        &mut self,
        memory: &mut Memory,
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) -> Result<(), String> {
        if self.halted {
            return Err(self
                .halt_error
                .clone()
                .unwrap_or_else(|| "CPU is halted".to_string()));
        }
        // Handle blocking wait-for-key (Fx0A)
        if let Some(reg) = self.waiting_for_key {
            if let Some(key) = input.get_pressed_key() {
                self.v[reg as usize] = key;
                self.waiting_for_key = None;
            }
            return Ok(());
        }

        let opcode = match memory.read_word(self.pc) {
            Ok(op) => op,
            Err(e) => {
                let msg = format!("PC {:#06X} out of bounds: {}", self.pc, e);
                return Err(self.halt(msg));
            }
        };
        self.pc += 2;
        if let Err(e) = self.decode_and_execute(opcode, memory, display, input, timers) {
            return Err(self.halt(e));
        }
        Ok(())
    }

    fn decode_and_execute(
        &mut self,
        opcode: u16,
        memory: &mut Memory,
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) -> Result<(), String> {
        let n0 = ((opcode & 0xF000) >> 12) as u8;
        let x  = ((opcode & 0x0F00) >> 8)  as u8;
        let y  = ((opcode & 0x00F0) >> 4)  as u8;
        let n  =  (opcode & 0x000F)         as u8;
        let kk =  (opcode & 0x00FF)         as u8;
        let nnn = opcode & 0x0FFF;

        match (n0, x, y, n) {
            // 00E0 — CLS
            (0x0, 0x0, 0xE, 0x0) => {
                display.clear();
            }
            // 00EE — RET (guard: stack underflow)
            (0x0, 0x0, 0xE, 0xE) => {
                if self.sp == 0 {
                    return Err(format!("Stack underflow on RET at PC {:04X}", self.pc - 2));
                }
                self.sp -= 1;
                self.pc = self.stack[self.sp as usize];
            }
            // 0nnn — SYS (ignored on modern interpreters)
            (0x0, _, _, _) => {}

            // 1nnn — JP addr (guard: stay out of the reserved interpreter area)
            (0x1, _, _, _) => {
                if nnn < 0x200 {
                    return Err(format!(
                        "JP to reserved interpreter area {:#05X} at PC {:04X}",
                        nnn,
                        self.pc - 2
                    ));
                }
                self.pc = nnn;
            }
            // 2nnn — CALL addr (guard: stack overflow + reserved area)
            (0x2, _, _, _) => {
                if nnn < 0x200 {
                    return Err(format!(
                        "CALL to reserved interpreter area {:#05X} at PC {:04X}",
                        nnn,
                        self.pc - 2
                    ));
                }
                if (self.sp as usize) >= self.stack.len() {
                    return Err(format!("Stack overflow on CALL at PC {:04X}", self.pc - 2));
                }
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            // 3xkk — SE Vx, kk
            (0x3, _, _, _) => {
                if self.v[x as usize] == kk {
                    self.pc += 2;
                }
            }
            // 4xkk — SNE Vx, kk
            (0x4, _, _, _) => {
                if self.v[x as usize] != kk {
                    self.pc += 2;
                }
            }
            // 5xy0 — SE Vx, Vy
            (0x5, _, _, 0x0) => {
                if self.v[x as usize] == self.v[y as usize] {
                    self.pc += 2;
                }
            }
            // 6xkk — LD Vx, kk
            (0x6, _, _, _) => {
                self.v[x as usize] = kk;
            }
            // 7xkk — ADD Vx, kk
            (0x7, _, _, _) => {
                self.v[x as usize] = self.v[x as usize].wrapping_add(kk);
            }

            // 8xy_ — arithmetic / logic
            (0x8, _, _, 0x0) => { self.v[x as usize]  = self.v[y as usize]; }
            (0x8, _, _, 0x1) => { self.v[x as usize] |= self.v[y as usize]; self.v[0xF] = 0; }
            (0x8, _, _, 0x2) => { self.v[x as usize] &= self.v[y as usize]; self.v[0xF] = 0; }
            (0x8, _, _, 0x3) => { self.v[x as usize] ^= self.v[y as usize]; self.v[0xF] = 0; }
            (0x8, _, _, 0x4) => {
                let (res, carry) = self.v[x as usize].overflowing_add(self.v[y as usize]);
                self.v[x as usize] = res;
                self.v[0xF] = carry as u8;
            }
            (0x8, _, _, 0x5) => {
                let (res, borrow) = self.v[x as usize].overflowing_sub(self.v[y as usize]);
                self.v[x as usize] = res;
                self.v[0xF] = (!borrow) as u8;
            }
            (0x8, _, _, 0x6) => {
                let lsb = self.v[x as usize] & 0x1;
                self.v[x as usize] >>= 1;
                self.v[0xF] = lsb;
            }
            (0x8, _, _, 0x7) => {
                let (res, borrow) = self.v[y as usize].overflowing_sub(self.v[x as usize]);
                self.v[x as usize] = res;
                self.v[0xF] = (!borrow) as u8;
            }
            (0x8, _, _, 0xE) => {
                let msb = (self.v[x as usize] & 0x80) >> 7;
                self.v[x as usize] <<= 1;
                self.v[0xF] = msb;
            }

            // 9xy0 — SNE Vx, Vy
            (0x9, _, _, 0x0) => {
                if self.v[x as usize] != self.v[y as usize] {
                    self.pc += 2;
                }
            }
            // Annn — LD I, addr
            (0xA, _, _, _) => {
                self.i = nnn;
            }
            // Bnnn — JP V0, addr (12-bit wrap; guard reserved area)
            (0xB, _, _, _) => {
                let target = (nnn + self.v[0] as u16) & 0xFFF;
                if target < 0x200 {
                    return Err(format!(
                        "Bnnn jump into reserved interpreter area {:#05X} at PC {:04X}",
                        target,
                        self.pc - 2
                    ));
                }
                self.pc = target;
            }
            // Cxkk — RND Vx, kk
            (0xC, _, _, _) => {
                let rnd: u8 = rand::thread_rng().gen();
                self.v[x as usize] = rnd & kk;
            }
            // Dxyn — DRW Vx, Vy, n
            (0xD, _, _, _) => {
                let xp = self.v[x as usize] as usize;
                let yp = self.v[y as usize] as usize;
                self.v[0xF] = 0;
                for row in 0..n {
                    let byte = memory.read_byte(self.i + row as u16).map_err(|e| {
                        format!("Dxyn sprite read OOB at I={:#06X} row {}: {}", self.i, row, e)
                    })?;
                    if display.draw_byte(xp, yp + row as usize, byte) {
                        self.v[0xF] = 1;
                    }
                }
            }

            // Ex9E — SKP Vx
            (0xE, _, 0x9, 0xE) => {
                if input.is_key_pressed(self.v[x as usize]) {
                    self.pc += 2;
                }
            }
            // ExA1 — SKNP Vx
            (0xE, _, 0xA, 0x1) => {
                if !input.is_key_pressed(self.v[x as usize]) {
                    self.pc += 2;
                }
            }

            // Fx07 — LD Vx, DT
            (0xF, _, 0x0, 0x7) => {
                self.v[x as usize] = timers.delay;
            }
            // Fx0A — LD Vx, K  (blocking)
            (0xF, _, 0x0, 0xA) => {
                self.waiting_for_key = Some(x);
            }
            // Fx15 — LD DT, Vx
            (0xF, _, 0x1, 0x5) => {
                timers.delay = self.v[x as usize];
            }
            // Fx18 — LD ST, Vx
            (0xF, _, 0x1, 0x8) => {
                timers.sound = self.v[x as usize];
            }
            // Fx1E — ADD I, Vx
            (0xF, _, 0x1, 0xE) => {
                self.i = self.i.wrapping_add(self.v[x as usize] as u16);
            }
            // Fx29 — LD F, Vx  (point I at font sprite for digit Vx)
            (0xF, _, 0x2, 0x9) => {
                self.i = (self.v[x as usize] & 0xF) as u16 * 5;
            }
            // Fx33 — LD B, Vx  (BCD at I, I+1, I+2)
            (0xF, _, 0x3, 0x3) => {
                let val = self.v[x as usize];
                memory.write_byte(self.i, val / 100).map_err(|e| {
                    format!("Fx33 BCD write OOB at I={:#06X}: {}", self.i, e)
                })?;
                memory.write_byte(self.i + 1, (val / 10) % 10).map_err(|e| {
                    format!("Fx33 BCD write OOB at I+1={:#06X}: {}", self.i + 1, e)
                })?;
                memory.write_byte(self.i + 2, val % 10).map_err(|e| {
                    format!("Fx33 BCD write OOB at I+2={:#06X}: {}", self.i + 2, e)
                })?;
            }
            // Fx55 — LD [I], Vx  (store V0..Vx)
            (0xF, _, 0x5, 0x5) => {
                for reg in 0..=(x as usize) {
                    memory.write_byte(self.i + reg as u16, self.v[reg]).map_err(|e| {
                        format!("Fx55 store OOB at I+{}={:#06X}: {}", reg, self.i + reg as u16, e)
                    })?;
                }
            }
            // Fx65 — LD Vx, [I]  (load V0..Vx)
            (0xF, _, 0x6, 0x5) => {
                for reg in 0..=(x as usize) {
                    let b = memory.read_byte(self.i + reg as u16).map_err(|e| {
                        format!("Fx65 load OOB at I+{}={:#06X}: {}", reg, self.i + reg as u16, e)
                    })?;
                    self.v[reg] = b;
                }
            }

            _ => {
                return Err(format!("Unknown opcode: {:04X} at PC {:04X}", opcode, self.pc - 2));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH};
    use crate::input::Input;
    use crate::timers::Timers;

    fn harness(opcode: u16) -> (Cpu, Memory, Display, Input, Timers) {
        let mut cpu = Cpu::new();
        let mut mem = Memory::new();
        // load opcode at 0x200
        mem.ram[0x200] = (opcode >> 8) as u8;
        mem.ram[0x200 + 1] = (opcode & 0xFF) as u8;
        cpu.pc = 0x200;
        (cpu, mem, Display::new(), Input::new(), Timers::new())
    }

    #[test]
    fn op_8xy4_carry_sets_vf() {
        let (mut cpu, mut mem, mut disp, input, mut timers) = harness(0x8124);
        cpu.v[1] = 200;
        cpu.v[2] = 100;
        cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers).unwrap();
        assert_eq!(cpu.v[1], 44);
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn op_8xy4_no_carry_clears_vf() {
        let (mut cpu, mut mem, mut disp, input, mut timers) = harness(0x8124);
        cpu.v[1] = 10;
        cpu.v[2] = 20;
        cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers).unwrap();
        assert_eq!(cpu.v[1], 30);
        assert_eq!(cpu.v[0xF], 0);
    }

    #[test]
    fn op_dxyn_wraps_around_screen() {
        // V0=63 (right edge), V1=31 (bottom edge), sprite 0x80 (single left pixel)
        let (mut cpu, mut mem, mut disp, input, mut timers) = harness(0xD011);
        cpu.v[0] = (DISPLAY_WIDTH - 1) as u8;
        cpu.v[1] = (DISPLAY_HEIGHT - 1) as u8;
        cpu.i = 0x300;
        mem.ram[0x300] = 0xFF; // 8 pixels starting at x=63 -> wraps to x=0..6
        cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers).unwrap();
        assert_eq!(cpu.v[0xF], 0);
        // pixel at (63,31) and wrapped pixel at (0,31) must be on
        assert!(disp.get_pixel(63, 31));
        assert!(disp.get_pixel(0, 31));
        // second draw of same sprite collides -> VF=1
        cpu.pc = 0x200;
        cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers).unwrap();
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn op_dxyn_oob_propagates_and_halts() {
        let (mut cpu, mut mem, mut disp, input, mut timers) = harness(0xD012);
        cpu.i = 0xFFF; // row 1 reads past 0xFFF -> OOB
        let res = cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers);
        assert!(res.is_err());
        assert!(cpu.halted);
    }

    #[test]
    fn op_bnnn_masks_to_12_bits() {
        // V0=0xFF, nnn=0xFFF -> (0xFFF+0xFF)&0xFFF = 0x0FE
        let (mut cpu, mut mem, mut disp, input, mut timers) = harness(0xBFFF);
        cpu.v[0] = 0xFF;
        cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers).unwrap();
        assert_eq!(cpu.pc, 0x0FE);
    }

    #[test]
    fn pc_oob_halts_cpu() {
        let mut cpu = Cpu::new();
        let mut mem = Memory::new();
        let mut disp = Display::new();
        let input = Input::new();
        let mut timers = Timers::new();
        cpu.pc = 0xFFF; // read_word needs 2 bytes -> OOB
        let res = cpu.execute_cycle(&mut mem, &mut disp, &input, &mut timers);
        assert!(res.is_err());
        assert!(cpu.halted);
        assert!(cpu.halt_error.is_some());
    }
}
