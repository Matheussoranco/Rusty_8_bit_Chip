use minifb::{Key, Window};

/// CHIP-8 has a 16-key hexadecimal keypad (0x0–0xF).
/// Keyboard mapping:
///   CHIP-8  Keyboard
///   1 2 3 C  →  1 2 3 4
///   4 5 6 D  →  Q W E R
///   7 8 9 E  →  A S D F
///   A 0 B F  →  Z X C V
pub struct Input {
    pub keys: [bool; 16],
}

const KEY_MAP: [(usize, Key); 16] = [
    (0x0, Key::X),
    (0x1, Key::Key1),
    (0x2, Key::Key2),
    (0x3, Key::Key3),
    (0x4, Key::Q),
    (0x5, Key::W),
    (0x6, Key::E),
    (0x7, Key::A),
    (0x8, Key::S),
    (0x9, Key::D),
    (0xA, Key::Z),
    (0xB, Key::C),
    (0xC, Key::Key4),
    (0xD, Key::R),
    (0xE, Key::F),
    (0xF, Key::V),
];

impl Input {
    pub fn new() -> Self {
        Input { keys: [false; 16] }
    }

    pub fn update(&mut self, window: &Window) {
        for (chip8_key, kb_key) in &KEY_MAP {
            self.keys[*chip8_key] = window.is_key_down(*kb_key);
        }
    }

    pub fn is_key_pressed(&self, key: u8) -> bool {
        self.keys[key as usize & 0xF]
    }

    /// Returns the first pressed key index, or None.
    pub fn get_pressed_key(&self) -> Option<u8> {
        self.keys.iter().position(|&k| k).map(|i| i as u8)
    }
}
