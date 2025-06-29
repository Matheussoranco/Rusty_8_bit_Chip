use crate::memory::Memory;
use crate::display: :Display;
use crate::input: :Input;
use crate::timers: :Timers;
use rand::Rng;

pub struct Cpu {
    pub v: [u8; 16], // Registradores V0-VF
    pub i: u16, // Registrador de índice
    pub pc: u16, // Contador de programa
    pub sp: u8, // Ponteiro de pilha
    pub stack: [u16, 16], // Pilha de chamadas
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            v: [0; 16],
            i: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16],
        }
    }

    pub fn execute_cycle(
        &mut self,
        memory: &mut Memory,
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) {
        let opcode = memory.read_word(self.pc);
        self.pc += 2;

        self.decode_and_execute(opcode, memory, display, input, timers);
    }

    fn decode_and_execute(
        &mut self,
        opcode: u16,
        memory: &mut Memory;
        display: &mut Display,
        input: &Input,
        timers: &mut Timers,
    ) {
        let nibbles = (
            ((opcode & 0xF000) >> 12) as u8,
            ((opcode & 0x0F00) >> 8) as u8,
            ((opcode & 0x00F0) >> 4) as u8,
            (opcode & 0x000F) as u8,
        );

        match nibbles {
            (0x0, 0x0, 0xE, 0x0) => {
                display.clear();
            },
            (0x0, 0x0, 0xE, 0xE) => {
                self.sp -= 1;
                self.pc = self.stack[self.sp as usize];
            },
            (0x1, _, _, _) => {
                self.pc = opcode & 0x0FFF;
            },
            (0x2, _, _, _) => {
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = opcode & 0x0FFF;
            },
            // 3xkk - SE Vx == kk: Pula a próxima instrução se Vx == kk
            (0x3, x, _, _) => {
                if self.v[x as usize] == (opcode & 0x00FF) as u8 {
                    self.pc += 2;
                }
            },
            // 4xkk - SNE Vx, kk: Pula a próxima instrução se Vx != kk
            (0x4, x, _, _) => {
                if self.v[x as usize] != (opcode & 0x00FF) as u8 {
                    self.pc += 2;
                }
            },
            // 5xy0 - SE Vx, Vy: Pula a próxima instrução se Vx == Vy
            (0x5, x, y, 0x0) => {
                if self.v[x as usize] == self.v[y as usize] {
                    self.pc += 2;
                }
            },
            // 6xkk - LD Vx, kk: Define Vx = kk
            (0x6, x, _, _) => {
                self.v[x as usize] = (opcode & 0x00FF) as u8;
            },
            // 7xkk - ADD Vx, kk: Soma kk a Vx
            (0x7, x, _, _) => {
                self.v[x as usize] = self.v[x as usize].wrapping_add((opcode & 0x00FF) as u8);
            },
            // 8xy0 - LD Vx, Vy: Vx = Vy
            (0x8, x, y, 0x0) => {
                self.v[x as usize] = self.v[y as usize];
            },
            // 8xy1 - OR Vx, Vy
            (0x8, x, y, 0x1) => {
                self.v[x as usize] |= self.v[y as usize];
            },
            // 8xy2 - AND Vx, Vy
            (0x8, x, y, 0x2) => {
                self.v[x as usize] &= self.v[y as usize];
            },
            // 8xy3 - XOR Vx, Vy
            (0x8, x, y, 0x3) => {
                self.v[x as usize] ^= self.v[y as usize];
            },
            // 8xy4 - ADD Vx, Vy (com carry)
            (0x8, x, y, 0x4) => {
                let (res, carry) = self.v[x as usize].overflowing_add(self.v[y as usize]);
                self.v[0xF] = if carry { 1 } else { 0 };
                self.v[x as usize] = res;
            },
            // 8xy5 - SUB Vx, Vy (com borrow)
            (0x8, x, y, 0x5) => {
                let (res, borrow) = self.v[x as usize].overflowing_sub(self.v[y as usize]);
                self.v[0xF] = if borrow { 0 } else { 1 };
                self.v[x as usize] = res;
            },
            // 8xy6 - SHR Vx {, Vy}
            (0x8, x, _, 0x6) => {
                self.v[0xF] = self.v[x as usize] & 0x1;
                self.v[x as usize] >>= 1;
            },
            // 8xy7 - SUBN Vx, Vy
            (0x8, x, y, 0x7) => {
                let (res, borrow) = self.v[y as usize].overflowing_sub(self.v[x as usize]);
                self.v[0xF] = if borrow { 0 } else { 1 };
                self.v[x as usize] = res;
            },
            // 8xyE - SHL Vx {, Vy}
            (0x8, x, _, 0xE) => {
                self.v[0xF] = (self.v[x as usize] & 0x80) >> 7;
                self.v[x as usize] <<= 1;
            },
            // 9xy0 - SNE Vx, Vy
            (0x9, x, y, 0x0) => {
                if self.v[x as usize] != self.v[y as usize] {
                    self.pc += 2;
                }
            },
            // Annn - LD I, addr
            (0xA, _, _, _) => {
                self.i = opcode & 0x0FFF;
            },
            // Bnnn - JP V0, addr
            (0xB, _, _, _) => {
                self.pc = (opcode & 0x0FFF) + self.v[0] as u16;
            },
            // Cxkk - RND Vx, kk
            (0xC, x, _, _) => {
                let mut rng = rand::thread_rng();
                let rnd: u8 = rng.gen();
                self.v[x as usize] = rnd & (opcode & 0x00FF) as u8;
            },
            (0xD, x, y, n) => {
                let x_pos = self.v[x as usize] as usize;
                let y_pos = self.v[y as usize] as usize;
                self.v[0xF] = 0;

                for byte in 0..n{
                    let sprite_byte = memory.read_byte(self.i + byte as u16);
                    if display.draw_byte(x_pos, y_pos + byte as usize, sprite_byte) {
                        self.v[0xF] = 1;
                    }
                }
                display.update();
            },
            _ => {
                println!("Opcode não implementado: {:04X}", opcode);
            }
        }
    }
}