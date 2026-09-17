pub struct Memory {
    pub ram: [u8; 4096],
}

pub const MEM_SIZE: usize = 4096;

impl Memory {
    pub fn new() -> Self {
        let mut memory = Memory { ram: [0; 4096] };

        let font: [u8; 80] = [
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80, // F
        ];
        memory.ram[..font.len()].copy_from_slice(&font);

        memory
    }

    pub fn read_byte(&self, addr: u16) -> Result<u8, String> {
        let a = addr as usize;
        self.ram.get(a).copied().ok_or_else(|| format!("read OOB: {:#06X}", addr))
    }

    pub fn read_word(&self, addr: u16) -> Result<u16, String> {
        let a = addr as usize;
        if a + 1 >= MEM_SIZE {
            return Err(format!("read OOB: {:#06X}", addr));
        }
        Ok((self.ram[a] as u16) << 8 | self.ram[a + 1] as u16)
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), String> {
        let a = addr as usize;
        match self.ram.get_mut(a) {
            Some(cell) => { *cell = value; Ok(()) }
            None => Err(format!("write OOB: {:#06X}", addr)),
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) -> Result<(), String> {
        let end = 0x200 + rom.len();
        if end > MEM_SIZE {
            return Err(format!("ROM too large: {} bytes (max {})", rom.len(), MEM_SIZE - 0x200));
        }
        self.ram[0x200..end].copy_from_slice(rom);
        Ok(())
    }
}
