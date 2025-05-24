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