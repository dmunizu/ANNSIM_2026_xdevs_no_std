# Examples for xDEVS no_std ANNSIM 2026

This repository contains an example of the co-simulation flow allowed by the [xDEVS no_std](https://github.com/iscar-ucm/xdevs_no_std.rs) DEVS simulator written in Rust.

## Pre-requisites

This example has been tested in the following systems:
- Fedora Linux 43
- Windows 11 (needs the [CP210x driver](https://www.silabs.com/software-and-tools/usb-to-uart-bridge-vcp-drivers) for serial UART to USB communication)
  
If you want to run the provided example, you first need to install the following dependencies:

### Rust 1.87 or higher

To install Rust on your machine, refer to [the official Rust page](https://www.rust-lang.org/tools/install).
You can check the Rust version on your machine by running the following command in your terminal:

```bash
rustc --version
```

If your current version is older than `rustc 1.87.0`, you can upgrade the toolchain with:

```bash
rustup update
```

### `riscv32imac-unknown-none-elf` target

In this example, we are working with the [`ESP32-C6-DevKitM-1`](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitm-1/index.html) evaluation kit. This board contains an `ESP32-C6FH4` chip that complies with the RISCV32 IMAC standard. This means that our board is:

- A RISC-V target that implements the Base Integer Instruction Set for 32 bits (RV32I).
- It includes the Standard Extension for Integer Multiplication and Division (M).
- It includes the Standard Extension for Atomic Instructions (A).
- It includes the Standard Extension for Compressed Instructions (C).

We need to install the Rust target to build code for this board. You can check if it is already installed by running the following command:

```bash
rustup target list 
```

Within a long list of available targets, you should see something like:

<pre>
<b>riscv32imac-unknown-none-elf (installed)</b>
</pre>

If you don't have it installed yet, you can run the following command:

```bash
rustup target add riscv32imac-unknown-none-elf
```

### `espflash`

Serial flasher utilities for Espressif devices:

```bash
cargo install espflash --locked
```

### `probe-rs`

It allows for on-chip debugging of the ESP32 project. You can refer to the [installation](https://probe.rs/docs/getting-started/installation/) guide available on the probe-rs website.

### Components for the ESP32-C6 implementation

To execute the example in an `ESP32-C6-DevKitM-1` you will need to connect the board to your PC through UART and the following components:
- LED: connected to the GPIO9.
- BME280 sensor: connected through I2C to GPIO6 (SDA) and GPIO7 (SCL)

### Visual Studio Code (recommended)

VS Code is the recommended IDE to work with this example. This repository has been configured to make working with Rust Embedded nearly as easy as developing native applications. To install it, follow the instructions from [the official VSCode webpage](https://code.visualstudio.com).

### VSCode extensions (for those working with VSCode)

If you want to use VSCode (which is the recommended way), then you will need to install the following VSCode extensions to make it work:

- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer): This official extension provides support for the Rust programming language.
- [Debugger for probe-rs](https://marketplace.visualstudio.com/items?itemName=probe-rs.probe-rs-debugger): This extension provides support for `probe-rs` debugging.
- [Serial monitor](https://marketplace.visualstudio.com/items?itemName=ms-vscode.vscode-serial-monitor) (optional): This official extension provides support for serial communication from VSCode itself, while not necessary, it simplifies the serial communication with the ESP32-C6 board through UART.

### Python 3 (optional)

The computer simulation has default values mimicking those of our test system. However, if you want it to mirror you implementation, a Python script that generates configuration values for it is provided in this repository. The `numpy` and `pyserial` packages are necessary for the script to run.

## Project structure

This project is divided in three separate Rust crates:

- `common_logic`: Library crate containing the DEVS models representing the common logic that will remain in both the PC simulation and ESP32 implementation.
- `esp_32_c6_project`: The crate implementing the common logic on the ESP32‑C6 board, interacting with real hardware through async tasks.
- `pc_simulation`: A crate providing a full computer‑based simulation in which the common logic interacts with DEVS models that replace the physical components connected to the ESP32‑C6.

## Run and debug the example

### esp32_c6_project

The ESP32 project can be executed directly by just selecting Run in `main.rs` within VSCode or by executing:

```bash
cargo run
```
To debug it, a VSCode launch.json has been configured with actions to both Launch the example with probe-rs debugging or Attach probe-rs to an already running example.

### pc_simulation

The computer simulation can be executed by selecting Run in `main.rs` within VSCode or by executing:

```bash
cargo run
```

The simulation can be debugged by selecting Debug in `main.rs` within VSCode:

## Interfacing with the system

The example, in both the simulation and real implementation, interacts with the user through commands that are input through the terminal (`pc_simulation`) or UART serial communication (`esp32_c6_project`, the UART messages must end with `\n` character). The commands are the following:
- `TEMP ON` and `TEMP OFF`: Enable or disable the periodic acquisition of temperature (in °C) measurements each second.
- `HUM ON` and `HUM OFF`: Enable or disable the periodic acquisition of humidity (in %) measurements each second.
- `LED ON` and `LED OFF`: Turn the LED on or off.

Each command will trigger reports from the program with the measurement values or the LED state and the time (in microseconds) it took for the action to be processed.

In `pc_simulation` the reports are also stored in a Comma Separated Value (CSV) file with unit annotation to increase their readability.

The DEVs model execution type can be configured through the `SIM_TIME` constant at the top of each `main.rs`.

## Configuring `pc_simulation`
The models that simulate the sensors and LEDs within `pc_simulation` take normal distributions to generate measurement values and delays that mimic those of the real implementation. The normal distribution parameters can be configured through constants at the top of `main.rs`.

The default value for these parameters has been taken from our test system. If you want the simulation to mirror your own implementation, you can by using the `nd_creator.py` script in this repository:

```bash
python nd_creator.py
```

1. Run `esp_32_c6_project` on your ESP32-C6 board.
2. Execute `nd_creator.py`: The script should auto-detect the serial port your UART is connected to and call the commands to retrieve temperature, humidity and change the LED state. It will store the UART output in a CSV file like the one generated by `pc_simulation` and finally output the values you should input for `pc_simulation` to mimic your own system.
