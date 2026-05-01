mod memory;
mod display;
mod input;
mod timers;
mod audio;
mod cpu;
mod savestate;

use std::{env, fs, path::Path, time::{Duration, Instant}};
use minifb::{Key, Window, WindowOptions, Scale};
use display::{DISPLAY_WIDTH, DISPLAY_HEIGHT, SCALE};
use savestate::SaveState;

/// CPU cycles executed per second. 700 Hz is a common target for most ROMs.
const CPU_HZ: u64 = 700;
/// Timer and display refresh rate.
const TIMER_HZ: u64 = 60;
const SAVE_STATE_PATH: &str = "savestate.bin";

fn print_usage() {
    eprintln!("Usage: chip8 <rom_path>");
    eprintln!();
    eprintln!("Controls:");
    eprintln!("  CHIP-8 keypad → Keyboard");
    eprintln!("  1 2 3 C       → 1 2 3 4");
    eprintln!("  4 5 6 D       → Q W E R");
    eprintln!("  7 8 9 E       → A S D F");
    eprintln!("  A 0 B F       → Z X C V");
    eprintln!();
    eprintln!("  F5  — Save state");
    eprintln!("  F9  — Load state");
    eprintln!("  F1  — Reset");
    eprintln!("  Esc — Quit");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let rom = match fs::read(rom_path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to read ROM '{}': {}", rom_path, e);
            std::process::exit(1);
        }
    };

    let mut memory  = memory::Memory::new();
    let mut display = display::Display::new();
    let mut input   = input::Input::new();
    let mut timers  = timers::Timers::new();
    let mut cpu     = cpu::Cpu::new();
    let audio       = audio::Audio::new();

    memory.load_rom(&rom);

    let win_w = DISPLAY_WIDTH  * SCALE;
    let win_h = DISPLAY_HEIGHT * SCALE;

    let mut window = Window::new(
        &format!("CHIP-8 — {}", Path::new(rom_path).file_name().unwrap_or_default().to_string_lossy()),
        win_w,
        win_h,
        WindowOptions {
            scale: Scale::X1,
            ..WindowOptions::default()
        },
    ).expect("Failed to create window");

    window.limit_update_rate(None); // We manage our own timing.

    let cpu_period   = Duration::from_nanos(1_000_000_000 / CPU_HZ);
    let timer_period = Duration::from_nanos(1_000_000_000 / TIMER_HZ);

    let mut last_cpu_tick   = Instant::now();
    let mut last_timer_tick = Instant::now();

    // Track key states for edge-triggered save/load/reset
    let mut f1_prev  = false;
    let mut f5_prev  = false;
    let mut f9_prev  = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();

        // --- CPU ticks ---
        if now.duration_since(last_cpu_tick) >= cpu_period {
            cpu.execute_cycle(&mut memory, &mut display, &input, &mut timers);
            last_cpu_tick = now;
        }

        // --- Timer tick + display refresh at 60 Hz ---
        if now.duration_since(last_timer_tick) >= timer_period {
            timers.tick();
            audio.set_beeping(timers.is_beeping());

            // Render display when dirty
            if display.dirty {
                let buf = display.render_to_buffer();
                window
                    .update_with_buffer(&buf, win_w, win_h)
                    .expect("Window update failed");
                display.dirty = false;
            } else {
                window.update();
            }

            // Update input after window update (so key state is fresh)
            input.update(&window);

            // --- Edge-triggered hotkeys ---
            let f1  = window.is_key_down(Key::F1);
            let f5  = window.is_key_down(Key::F5);
            let f9  = window.is_key_down(Key::F9);

            if f1 && !f1_prev {
                println!("Reset");
                cpu     = cpu::Cpu::new();
                memory  = memory::Memory::new();
                display = display::Display::new();
                timers  = timers::Timers::new();
                memory.load_rom(&rom);
            }

            if f5 && !f5_prev {
                let state = SaveState::capture(&cpu, &memory, &display, &timers);
                match state.save_to_file(Path::new(SAVE_STATE_PATH)) {
                    Ok(_)  => println!("State saved to {}", SAVE_STATE_PATH),
                    Err(e) => eprintln!("Save failed: {}", e),
                }
            }

            if f9 && !f9_prev {
                match SaveState::load_from_file(Path::new(SAVE_STATE_PATH)) {
                    Ok(state) => {
                        state.restore(&mut cpu, &mut memory, &mut display, &mut timers);
                        println!("State loaded from {}", SAVE_STATE_PATH);
                    }
                    Err(e) => eprintln!("Load failed: {}", e),
                }
            }

            f1_prev = f1;
            f5_prev = f5;
            f9_prev = f9;

            last_timer_tick = now;
        }

        // Yield the thread briefly to avoid 100% CPU spin
        std::thread::sleep(Duration::from_micros(100));
    }
}
