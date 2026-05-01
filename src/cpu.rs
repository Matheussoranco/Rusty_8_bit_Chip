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
        }
    }

    pub fn execute_cycle(
        &mut self,
        memory: &mut Memory,
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) {
        // Handle blocking wait-for-key (Fx0A)
        if let Some(reg) = self.waiting_for_key {
            if let Some(key) = input.get_pressed_key() {
                self.v[reg as usize] = key;
                self.waiting_for_key = None;
            }
            return;
        }

        let opcode = memory.read_word(self.pc);
        self.pc += 2;
        self.decode_and_execute(opcode, memory, display, input, timers);
    }

    fn decode_and_execute(
        &mut self,
        opcode: u16,
        memory: &mut Memory,
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) {
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
            // 00EE — RET
            (0x0, 0x0, 0xE, 0xE) => {
                self.sp -= 1;
                self.pc = self.stack[self.sp as usize];
            }
            // 0nnn — SYS (ignored on modern interpreters)
            (0x0, _, _, _) => {}

            // 1nnn — JP addr
            (0x1, _, _, _) => {
                self.pc = nnn;
            }
            // 2nnn — CALL addr
            (0x2, _, _, _) => {
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
            // Bnnn — JP V0, addr
            (0xB, _, _, _) => {
                self.pc = nnn + self.v[0] as u16;
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
                    let byte = memory.read_byte(self.i + row as u16);
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
                memory.write_byte(self.i,     val / 100);
                memory.write_byte(self.i + 1, (val / 10) % 10);
                memory.write_byte(self.i + 2, val % 10);
            }
            // Fx55 — LD [I], Vx  (store V0..Vx)
            (0xF, _, 0x5, 0x5) => {
                for reg in 0..=(x as usize) {
                    memory.write_byte(self.i + reg as u16, self.v[reg]);
                }
            }
            // Fx65 — LD Vx, [I]  (load V0..Vx)
            (0xF, _, 0x6, 0x5) => {
                for reg in 0..=(x as usize) {
                    self.v[reg] = memory.read_byte(self.i + reg as u16);
                }
            }

            _ => {
                eprintln!("Unknown opcode: {:04X} at PC {:04X}", opcode, self.pc - 2);
            }
        }
    }
}
