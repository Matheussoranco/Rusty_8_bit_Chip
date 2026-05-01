pub const DISPLAY_WIDTH: usize = 64;
pub const DISPLAY_HEIGHT: usize = 32;
pub const SCALE: usize = 12;

const COLOR_ON: u32 = 0x00F8F8F2;  // bright white
const COLOR_OFF: u32 = 0x00282A36; // dark background

pub struct Display {
    pub pixels: [bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
    pub dirty: bool,
}

impl Display {
    pub fn new() -> Self {
        Display {
            pixels: [false; DISPLAY_WIDTH * DISPLAY_HEIGHT],
            dirty: false,
        }
    }

    pub fn clear(&mut self) {
        self.pixels = [false; DISPLAY_WIDTH * DISPLAY_HEIGHT];
        self.dirty = true;
    }

    /// Draw one byte of sprite data at (x, y). Returns true if any pixel was erased (collision).
    pub fn draw_byte(&mut self, x: usize, y: usize, byte: u8) -> bool {
        let mut collision = false;
        for bit in 0..8 {
            if (byte >> (7 - bit)) & 0x1 == 0 {
                continue;
            }
            let px = (x + bit) % DISPLAY_WIDTH;
            let py = y % DISPLAY_HEIGHT;
            let idx = py * DISPLAY_WIDTH + px;
            if self.pixels[idx] {
                collision = true;
            }
            self.pixels[idx] ^= true;
        }
        self.dirty = true;
        collision
    }

    pub fn render_to_buffer(&self) -> Vec<u32> {
        let w = DISPLAY_WIDTH * SCALE;
        let h = DISPLAY_HEIGHT * SCALE;
        let mut buf = vec![COLOR_OFF; w * h];

        for py in 0..DISPLAY_HEIGHT {
            for px in 0..DISPLAY_WIDTH {
                let color = if self.pixels[py * DISPLAY_WIDTH + px] {
                    COLOR_ON
                } else {
                    COLOR_OFF
                };
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        buf[(py * SCALE + sy) * w + (px * SCALE + sx)] = color;
                    }
                }
            }
        }
        buf
    }

    #[allow(dead_code)]
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        self.pixels[y * DISPLAY_WIDTH + x]
    }
}
