# PulseTTY
A **terminal-based music visualiser** that reacts to audio input (MP3, Mic or System Audio), and **visualises the frequencies** in the terminal. It has different modes, including **bars**, **waveform**, and a **spectrogram**.

### Download it [from crates.io](https://crates.io/crates/PulseTTY)!


https://github.com/user-attachments/assets/aa3cc2b5-8ee6-43da-a3c1-d243f209e7d4

| ![bars](https://github.com/user-attachments/assets/bbff94a8-aa56-4837-bd91-c43bac7ca7b0) | ![line](https://github.com/user-attachments/assets/951fd9ef-726c-42b3-b0cb-6be3bbff3650) | ![spectrum](https://github.com/user-attachments/assets/1dde9a92-ed8b-4404-a109-16bdf3541ddd) |
|:----------------------------------------------------------------------------------------:|:----------------------------------------------------------------------------------------:|:--------------------------------------------------------------------------------------------:|
|                                        Bars Mode                                         | Lines Mode                                                                               | Spectrum Analyser                                                                            |
## Features
- Runs as a CLI in the terminal
- Real-time audio visualisation, driven by audio input
- Supports multiple input modes:
  - System audio loopback (default)
  - MP3/Audio file playback
  - Microphone Input
- Multiple rendering modes (bars, line, spectrogram, or VU)
- Lightweight ASCII/Unicode terminal rendering
- Dynamically detects and fits to terminal size

## Installation
Currently, PulseTTY only supports Windows environments, due to underlying audio pipeline structuring relying upon WASAPI. You can install it in a few different ways.

### Via Cargo (recommended)
You must first have Rust and Cargo installed. You can find their installation instructions [here](https://rust-lang.org/tools/install/).

Now run:
```bash
cargo install pulsetty
```
This will globally install PulseTTY as a binary, allowing you to run it from any directory with `pulsetty`.

### Build From Source (for development)
You must first have Rust and Cargo installed. You can find their installation instructions [here](https://rust-lang.org/tools/install/).

Now run:
```bash
git clone https://github.com/MadAvidCoder/PulseTTY.git
cd PulseTTY
cargo build --release
```

This will create an executable binary, which you can run with:
```bash
./target/release/pulsetty
```

## Usage
> For a more detailed reference of all supported arguments, run `pulsetty --help`.

When run with no arguments, PulseTTY will display the `bars` rendering mode, using system audio, captured from the default output device. Use `q` or `Esc` to exit from the interface.

To select a specific output device, supply the `--device <INDEX>` option, with either its index or name. These can be obtained using the `--list-devices` option. To use an input device, use `--mic` or `-m`. A specific input device can be selected by supplying a parameter (`-m 0` or `-m "Microphone Array"`), which can be found by running `--list-mics`. To play from a file, supply the filename as an argument. (`pulsetty "path/to/a_song.mp3"`)

To preset a render mode, supply the `--mode <MODE>` option, with `bars`, `line`, `spectrogram` or `vu`. During runtime, you can cycle through modes by pressing `m`. You can also adjust input gain, with `-`/`+` and `0`, for compatability with quiet input sources.

## Troubleshooting
- If your terminal emulator doesn't support Unicode or ANSI Colour codes, you can use the `--ascii` or `--no-colour` flags to ensure compatibility. These can also be toggled with `a` and `c` while running.
- If PulseTTY fails to detect the height/width of the terminal, they can be overridden with `-c <N>` and `-H <ROWS`. You can also use the `--compact` flag to reduce size on smaller screens.
- If the rendering is lagging, try raising the frame delay, with `--frame-ms <MS>`. The default is `15ms`.
- If detecting a default audio capture fails, run `--list-devices` or `--list-mics` and provide the index of an active device.

## Tech Stack
- **Rust**
- **clap** (for CLI parsing)
- **crossterm** (terminal rendering + input handling)
- **rustfft** (FFT Transforms)
- **symphonia** (file decoding)
- **wasapi** (Windows live audio capture)

## Licence
PulseTTY is licensed under the [MIT Licence](LICENSE).

You are free to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of this software. You must include the original copyright and license notice in any copies or substantial portions of the project.

**There is no warranty**. PulseTTY is provided “as is”, without warranty of any kind. Use at your own risk.
