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
            // Implementar outras instruções
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