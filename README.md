# Rusty 8-bit Chip — CHIP-8 Emulator

A cycle-accurate CHIP-8 interpreter written in Rust. Targets the original 1977 RCA COSMAC VIP specification with CHIP-48/SUPER-CHIP divergences documented where they apply.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Hardware Specification](#hardware-specification)
3. [Memory Map](#memory-map)
4. [Instruction Set Architecture](#instruction-set-architecture)
5. [Opcode Reference Table](#opcode-reference-table)
6. [Execution Pipeline](#execution-pipeline)
7. [Display Subsystem](#display-subsystem)
8. [Input Subsystem](#input-subsystem)
9. [Timer Subsystem](#timer-subsystem)
10. [Audio Subsystem](#audio-subsystem)
11. [Save State Serialization](#save-state-serialization)
12. [Timing Model](#timing-model)
13. [Known Behavioral Quirks](#known-behavioral-quirks)
14. [Building and Running](#building-and-running)
15. [ROM Compatibility](#rom-compatibility)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        main loop (60 Hz)                    │
│                                                             │
│   ┌──────────┐    ┌──────────┐    ┌────────┐  ┌─────────┐  │
│   │   CPU    │───▶│  Memory  │    │Display │  │  Input  │  │
│   │ (700 Hz) │◀───│  4 KB    │    │ 64×32  │  │ 16-key  │  │
│   └────┬─────┘    └──────────┘    └───┬────┘  └────┬────┘  │
│        │                              │             │       │
│        │          ┌──────────┐        │             │       │
│        └─────────▶│  Timers  │        │             │       │
│                   │ DT / ST  │        ▼             │       │
│                   └────┬─────┘   ┌────────┐         │       │
│                        │         │ minifb │◀────────┘       │
│                        ▼         │ window │                 │
│                   ┌──────────┐   └────────┘                 │
│                   │  Audio   │                              │
│                   │  (cpal)  │                              │
│                   └──────────┘                              │
└─────────────────────────────────────────────────────────────┘
```

The emulator runs two independent time domains:

- **CPU domain** — 700 Hz instruction fetch-decode-execute loop.
- **System domain** — 60 Hz timer decrement, display blit, and input poll.

Both domains share a monotonic `Instant` clock; no threads are used. The main loop spins with a 100 µs sleep granularity, comparing elapsed time against each domain's period before dispatching work.

---

## Hardware Specification

| Parameter           | Value                          |
|---------------------|-------------------------------|
| Word size           | 8-bit data, 16-bit addresses  |
| Addressable memory  | 4096 bytes (12-bit address space) |
| General registers   | 16 × 8-bit (V0–VF)            |
| Address register    | 1 × 16-bit (I)                |
| Program counter     | 16-bit, initialized to 0x200  |
| Call stack          | 16 levels × 16-bit            |
| Stack pointer       | 8-bit index into stack array  |
| Display resolution  | 64 × 32 monochrome pixels     |
| Sprite format       | 1-bit-per-pixel, 8 px wide, 1–15 px tall |
| Keypad              | 16 keys (0x0–0xF)             |
| Delay timer         | 8-bit, decrements at 60 Hz    |
| Sound timer         | 8-bit, decrements at 60 Hz, buzzes while > 0 |
| Instruction width   | 16-bit fixed (big-endian)     |
| Clock               | 700 Hz (configurable via `CPU_HZ`) |

---

## Memory Map

```
0x000 ┬─────────────────────────────────
      │  Interpreter area (reserved)
      │  Font sprites: 16 glyphs × 5 bytes
      │  Stored at 0x000–0x04F
0x050 ┤  (rest of interpreter area, unused)
      │
0x200 ┬─────────────────────────────────
      │  ROM / User programs
      │  All programs load here.
      │  Maximum ROM size: 3584 bytes (0xE00)
0xFFF ┴─────────────────────────────────
```

### Font Sprite Layout

Each glyph occupies 5 bytes encoding an 8×5 pixel bitmap (only the high 4 bits of each byte are used, giving an effective 4×5 grid). Glyph N is located at address `N * 5`.

```
Glyph '0'         Binary            Hex
 ████             1111 0000         0xF0
 █  █             1001 0000         0x90
 █  █             1001 0000         0x90
 █  █             1001 0000         0x90
 ████             1111 0000         0xF0
```

---

## Instruction Set Architecture

CHIP-8 instructions are 16 bits wide, big-endian, always 2-byte aligned. The instruction stream is decoded by extracting four 4-bit nibbles:

```
 15  12 11   8  7   4  3   0
┌──────┬──────┬──────┬──────┐
│  n0  │  x   │  y   │  n   │
└──────┴──────┴──────┴──────┘

Derived fields:
  kk  = bits [7:0]   (immediate byte)
  nnn = bits [11:0]  (immediate address)
```

The decoder is implemented as a Rust `match` over the tuple `(n0, x, y, n)`, enabling the compiler to emit an efficient jump table for the common cases.

### Register Conventions

| Register | Alias | Usage |
|----------|-------|-------|
| V0–VE    | —     | General purpose |
| VF       | flag  | Carry, borrow, collision, shift-out bits — **written as a side effect by arithmetic and draw instructions; do not use as a general register** |

---

## Opcode Reference Table

All 35 canonical opcodes are implemented. Notation: `x`, `y` = nibble register indices; `kk` = 8-bit immediate; `nnn` = 12-bit address; `n` = nibble.

| Opcode   | Mnemonic        | Operation |
|----------|-----------------|-----------|
| `00E0`   | CLS             | Fill display buffer with 0 |
| `00EE`   | RET             | PC ← stack[--SP] |
| `1nnn`   | JP nnn          | PC ← nnn |
| `2nnn`   | CALL nnn        | stack[SP++] ← PC; PC ← nnn |
| `3xkk`   | SE Vx, kk       | if Vx == kk: PC += 2 |
| `4xkk`   | SNE Vx, kk      | if Vx != kk: PC += 2 |
| `5xy0`   | SE Vx, Vy       | if Vx == Vy: PC += 2 |
| `6xkk`   | LD Vx, kk       | Vx ← kk |
| `7xkk`   | ADD Vx, kk      | Vx ← Vx + kk (wrapping, no flag) |
| `8xy0`   | LD Vx, Vy       | Vx ← Vy |
| `8xy1`   | OR Vx, Vy       | Vx ← Vx \| Vy; VF ← 0 |
| `8xy2`   | AND Vx, Vy      | Vx ← Vx & Vy; VF ← 0 |
| `8xy3`   | XOR Vx, Vy      | Vx ← Vx ^ Vy; VF ← 0 |
| `8xy4`   | ADD Vx, Vy      | Vx ← Vx + Vy; VF ← carry |
| `8xy5`   | SUB Vx, Vy      | Vx ← Vx - Vy; VF ← NOT borrow |
| `8xy6`   | SHR Vx          | VF ← Vx & 1; Vx ← Vx >> 1 |
| `8xy7`   | SUBN Vx, Vy     | Vx ← Vy - Vx; VF ← NOT borrow |
| `8xyE`   | SHL Vx          | VF ← Vx >> 7; Vx ← Vx << 1 |
| `9xy0`   | SNE Vx, Vy      | if Vx != Vy: PC += 2 |
| `Annn`   | LD I, nnn       | I ← nnn |
| `Bnnn`   | JP V0, nnn      | PC ← nnn + V0 |
| `Cxkk`   | RND Vx, kk      | Vx ← random_u8() & kk |
| `Dxyn`   | DRW Vx, Vy, n   | XOR sprite I[0..n] at (Vx, Vy); VF ← collision |
| `Ex9E`   | SKP Vx          | if key[Vx] down: PC += 2 |
| `ExA1`   | SKNP Vx         | if key[Vx] up: PC += 2 |
| `Fx07`   | LD Vx, DT       | Vx ← delay_timer |
| `Fx0A`   | LD Vx, K        | Block until keypress; Vx ← key |
| `Fx15`   | LD DT, Vx       | delay_timer ← Vx |
| `Fx18`   | LD ST, Vx       | sound_timer ← Vx |
| `Fx1E`   | ADD I, Vx       | I ← I + Vx (wrapping) |
| `Fx29`   | LD F, Vx        | I ← font_base + (Vx & 0xF) * 5 |
| `Fx33`   | LD B, Vx        | mem[I..I+2] ← BCD(Vx) |
| `Fx55`   | LD [I], Vx      | mem[I..I+x] ← V0..Vx |
| `Fx65`   | LD Vx, [I]      | V0..Vx ← mem[I..I+x] |

---

## Execution Pipeline

Each call to `Cpu::execute_cycle` performs one instruction cycle:

```
execute_cycle()
    │
    ├─[waiting_for_key?]─── poll input → store key → clear wait flag → return
    │
    ├── fetch:   opcode = mem[PC] << 8 | mem[PC+1]
    ├── advance: PC += 2
    └── decode_and_execute(opcode)
            │
            ├── extract nibbles (n0, x, y, n), kk, nnn
            └── match (n0, x, y, n) → execute handler
```

### Blocking I/O — Fx0A

`Fx0A` is the only blocking instruction. Rather than spin-waiting on the CPU thread, the implementation stores the target register index in `waiting_for_key: Option<u8>`. On every subsequent cycle, `execute_cycle` polls `input.get_pressed_key()` before fetching the next opcode. When a key is detected the value is committed to the register and the field is cleared, resuming normal execution.

This design avoids any platform-specific blocking primitive and integrates cleanly with the single-threaded event loop.

---

## Display Subsystem

The display is a 64×32 monochrome framebuffer stored as `[bool; 2048]` (row-major, origin top-left).

### XOR Drawing (Dxyn)

Sprites are drawn by XOR-ing each set bit into the framebuffer:

```
for row in 0..n:
    byte = mem[I + row]
    for bit in 0..8:
        if byte[7 - bit] == 1:
            px = (Vx + bit) % 64     ← wraps horizontally
            py = (Vy + row) % 32     ← wraps vertically
            collision |= pixels[py][px]
            pixels[py][px] ^= 1

VF ← collision (any pixel erased)
```

Wrapping is modular on each axis independently per the original spec.

### Rendering Pipeline

The framebuffer is upscaled to a window of `64 × SCALE` by `32 × SCALE` pixels (SCALE = 12, giving 768 × 384) by nearest-neighbor expansion. Each logical pixel maps to a `SCALE × SCALE` block of `u32` ARGB values.

The window is only re-blitted when `display.dirty == true`, which is set by `draw_byte` or `clear`. This avoids redundant blits on cycles where the display is idle.

Color palette:

| State | Value      | Description       |
|-------|------------|-------------------|
| ON    | `0xF8F8F2` | Bright near-white |
| OFF   | `0x282A36` | Dark near-black   |

---

## Input Subsystem

The CHIP-8 keypad is a 4×4 hexadecimal grid (keys 0x0–0xF). The physical keyboard mapping follows the de-facto standard:

```
CHIP-8 keypad       Keyboard
┌───┬───┬───┬───┐   ┌───┬───┬───┬───┐
│ 1 │ 2 │ 3 │ C │   │ 1 │ 2 │ 3 │ 4 │
├───┼───┼───┼───┤   ├───┼───┼───┼───┤
│ 4 │ 5 │ 6 │ D │   │ Q │ W │ E │ R │
├───┼───┼───┼───┤   ├───┼───┼───┼───┤
│ 7 │ 8 │ 9 │ E │   │ A │ S │ D │ F │
├───┼───┼───┼───┤   ├───┼───┼───┼───┤
│ A │ 0 │ B │ F │   │ Z │ X │ C │ V │
└───┴───┴───┴───┘   └───┴───┴───┴───┘
```

Key state is a `[bool; 16]` array polled once per 60 Hz system tick via `minifb::Window::is_key_down`. This matches the original hardware's level-sensitive (not edge-triggered) key detection semantics.

---

## Timer Subsystem

Both timers are 8-bit down-counters that decrement at exactly 60 Hz regardless of CPU frequency.

```
tick() (called at 60 Hz):
    if delay > 0: delay -= 1
    if sound > 0: sound -= 1
```

- **Delay timer (DT):** General-purpose timing register. Programs read it via `Fx07` and write it via `Fx15`.
- **Sound timer (ST):** Drives the buzzer. The audio subsystem activates the tone stream while `ST > 0`.

The two domains (CPU at 700 Hz, timers at 60 Hz) are decoupled by the main loop's independent `Instant`-based scheduling.

---

## Audio Subsystem

The audio subsystem generates a 440 Hz sine wave using `cpal`. Architecture:

```
Audio::new()
    │
    ├── acquire default output device
    ├── query default output config (sample rate, channel count, sample format)
    ├── spawn output stream with closure over Arc<AtomicBool>
    │       closure: generates sin(2π × 440 × phase/sample_rate) × 0.15
    │                outputs silence when flag is false
    └── stream.play()

main loop: audio.set_beeping(timers.is_beeping())
    └── AtomicBool::store(Ordering::Relaxed)
```

The sample format dispatch handles `F32`, `I16`, and `U16` output formats to cover the full range of platform audio drivers. The `_stream` field is kept alive in the struct to prevent the stream from being dropped and silenced.

The `Arc<AtomicBool>` bridge between the main thread and the audio callback thread avoids any mutex contention on the hot audio path.

---

## Save State Serialization

The complete emulator state can be captured and restored at any point. Serialization uses `bincode` (little-endian binary encoding) over a `serde`-derived struct:

```rust
struct SaveState {
    v:                [u8; 16],
    i:                u16,
    pc:               u16,
    sp:               u8,
    stack:            [u16; 16],
    waiting_for_key:  Option<u8>,
    ram:              Vec<u8>,      // 4096 bytes
    pixels:           Vec<bool>,    // 2048 bools
    delay:            u8,
    sound:            u8,
}
```

Serialized size is approximately 6.2 KB per slot. States are written to `savestate.bin` in the working directory.

Hotkeys:

| Key | Action |
|-----|--------|
| F1  | Reset (reload ROM, reinitialize all state) |
| F5  | Save state to `savestate.bin` |
| F9  | Load state from `savestate.bin` |
| Esc | Quit |

---

## Timing Model

```
wall clock
    │
    ├── CPU timer: fires every  1 / 700 Hz ≈ 1.43 ms
    │       └── execute_cycle()
    │
    └── System timer: fires every  1 / 60 Hz ≈ 16.67 ms
            ├── timers.tick()
            ├── audio.set_beeping()
            ├── display blit (if dirty)
            ├── window.update()
            └── input.update()
```

The main loop sleeps for 100 µs between iterations. This gives a worst-case dispatch jitter of ±100 µs, which is negligible relative to both the 1.43 ms CPU period and the 16.67 ms system period.

CPU frequency can be adjusted by changing `CPU_HZ` in `src/main.rs`. Common values:

| Frequency | Behavior |
|-----------|----------|
| 500 Hz    | Conservative; matches early COSMAC VIP |
| 700 Hz    | Default; correct for most ROMs |
| 1000 Hz   | Faster games; some ROMs require this |
| 1500 Hz+  | High-speed; useful for debugging |

---

## Known Behavioral Quirks

Several CHIP-8 instructions have ambiguous or platform-dependent behavior. This implementation follows the original COSMAC VIP specification:

### 8xy1 / 8xy2 / 8xy3 — Logic Operations Reset VF

The original interpreter resets VF to 0 after OR/AND/XOR. Many ROMs depend on this. CHIP-48 and SUPER-CHIP did not reset VF, which broke some programs.

### 8xy6 / 8xyE — Shift Operates on Vx Directly

The original spec shifts Vy and stores the result in Vx. This implementation (and most modern interpreters) shifts Vx in-place, ignoring Vy. ROMs targeting the original behavior may be affected.

### Fx55 / Fx65 — I Is Not Modified

The original COSMAC VIP interpreter incremented I during the store/load loop (`I += 1` per register). This implementation leaves I unchanged after the operation, matching CHIP-48 behavior. Some older ROMs depend on I being mutated.

### Dxyn — Wrap-Around Clipping

Sprites that extend beyond the display boundary wrap around to the opposite edge, per the original spec. SUPER-CHIP clips instead of wrapping.

### Bnnn — Jump with Offset Uses V0

Uses `PC = nnn + V0`. SUPER-CHIP's variant (`BXNN`) uses `PC = XNN + VX`. This emulator implements the original form.

---

## Building and Running

### Prerequisites

- Rust 1.70+ (2021 edition)
- A C compiler (required by `cpal`'s audio backend on Windows)

### Build

```sh
cargo build --release
```

The optimized binary is placed at `target/release/chip8.exe` (Windows) or `target/release/chip8` (Unix).

### Run

```sh
cargo run --release -- path/to/rom.ch8
```

Or directly:

```sh
./target/release/chip8 path/to/rom.ch8
```

### Dependencies

| Crate      | Version | Purpose                              |
|------------|---------|--------------------------------------|
| `minifb`   | 0.24    | Cross-platform window + framebuffer  |
| `cpal`     | 0.15    | Cross-platform audio I/O             |
| `bincode`  | 1.3     | Binary serialization for save states |
| `serde`    | 1.0     | Derive macros for serialization      |
| `rand`     | 0.8     | PRNG for `Cxkk` (RND instruction)   |

---

## ROM Compatibility

Tested against the standard CHIP-8 test suite. The following categories of ROMs are supported:

- Classic arcade games (Space Invaders, Pong, Tetris, Breakout)
- Keypad input programs
- Timer-dependent programs
- BCD and arithmetic tests
- Sprite drawing and collision tests

ROMs targeting SUPER-CHIP extensions (128×64 display, scrolling, 16×16 sprites) are **not** supported by this implementation.

---

## Module Index

```
src/
├── main.rs         Entry point; main loop; timing; hotkeys
├── cpu.rs          Fetch-decode-execute; all 35 opcodes
├── memory.rs       4 KB RAM; font loader; ROM loader
├── display.rs      64×32 framebuffer; XOR draw; upscale blit
├── input.rs        16-key poll; keyboard mapping
├── timers.rs       60 Hz delay and sound timer
├── audio.rs        cpal sine-wave beep; AtomicBool control
└── savestate.rs    bincode snapshot capture and restore
```

---

## References

- Cowgod's CHIP-8 Technical Reference (1997)
- CHIP-8 Research Facility — Tobias V. Langhoff (2021)
- *BYTE Magazine*, December 1978 — Joseph Weisbecker, "An Easy Programming System"
- RCA COSMAC VIP Instruction Manual (1977)
