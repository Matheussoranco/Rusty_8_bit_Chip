pub struct Timers {
    pub delay: u8,
    pub sound: u8,
}

impl Timers {
    pub fn new() -> Self {
        Timers { delay: 0, sound: 0 }
    }

    /// Decrement both timers at 60 Hz.
    pub fn tick(&mut self) {
        if self.delay > 0 {
            self.delay -= 1;
        }
        if self.sound > 0 {
            self.sound -= 1;
        }
    }

    pub fn is_beeping(&self) -> bool {
        self.sound > 0
    }
}
