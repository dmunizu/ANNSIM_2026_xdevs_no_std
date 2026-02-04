#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use common_logic::{Command, CommonLogicInput, CommonLogicOutput};

use bme280_rs::{AsyncBme280, Configuration, Oversampling, SensorMode};
use embassy_executor::Spawner;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Delay, Instant, with_deadline};
use embassy_time::{Duration, Timer};

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::I2c;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{UartRx, UartTx};
use esp_hal::{Async, i2c};
use xdevs::simulator::Config;
use xdevs::traits::AsyncInput;

use core::fmt::Write;
use heapless::String;

// FIFO read buffer size
const READ_BUF_SIZE: usize = 20;
// Input Channel enum and size
enum ModelInputs {
    Command(Command),
    TempReading(f64),
    HumReading(f64),
    LedConfirmation(bool),
}
const IN_QUEUE_SIZE: usize = 16;
type MutexType = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
static IN_CHANNEL: Channel<MutexType, ModelInputs, IN_QUEUE_SIZE> = Channel::new();
// Output Channel enum and size
#[derive(Clone, PartialEq)]
enum ModelOutputs {
    TempRequest(bool),
    HumRequest(bool),
    LedCommand(bool),
    TempReport((f64, f64)),
    HumReport((f64, f64)),
    LedReport((bool, f64)),
}
const OUT_QUEUE_SIZE: usize = 16;
static OUT_CHANNEL: PubSubChannel<MutexType, ModelOutputs, OUT_QUEUE_SIZE, 3, 1> =
    PubSubChannel::new();

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
// Task that reads data from UART
async fn read_task(mut rx: UartRx<'static, Async>) {
    const MAX_BUFFER_SIZE: usize = 10 * READ_BUF_SIZE + 16;
    let mut rbuf: [u8; MAX_BUFFER_SIZE] = [0u8; MAX_BUFFER_SIZE];
    let mut offset = 0;
    loop {
        let r = embedded_io_async::Read::read(&mut rx, &mut rbuf[offset..]).await;
        match r {
            Ok(len) => {
                offset += len;
                let string = core::str::from_utf8(&rbuf[..offset]).unwrap().trim();
                // Wait for newline delimiter before processing
                if !rbuf[..offset].contains(&b'\n') {
                    continue;
                }
                let cmd = match string {
                    "TEMP ON" => Command::TempOn,
                    "TEMP OFF" => Command::TempOff,
                    "HUM ON" => Command::HumOn,
                    "HUM OFF" => Command::HumOff,
                    "LED ON" => Command::LedOn,
                    "LED OFF" => Command::LedOff,
                    &_ => {
                        esp_println::println!("Unknown command: {}", string);
                        offset = 0;
                        continue;
                    }
                };
                IN_CHANNEL.send(ModelInputs::Command(cmd)).await;
                offset = 0;
            }
            Err(e) => esp_println::println!("RX Error: {:?}", e),
        }
    }
}

#[embassy_executor::task]
// Task that reads data from the BME280 sensor
async fn sensor_task(i2c: I2c<'static, Async>) {
    // Subscribe to output channel
    let mut sub = OUT_CHANNEL.subscriber().unwrap();

    // Create BME280 sensor instance
    let mut bme280 = AsyncBme280::new(i2c, Delay);

    // Initialize the BME280 sensor
    bme280.init().await.unwrap();
    bme280
        .set_sampling_configuration(
            Configuration::default()
                .with_temperature_oversampling(Oversampling::Oversample8)
                .with_pressure_oversampling(Oversampling::Skip)
                .with_humidity_oversampling(Oversampling::Oversample8)
                .with_sensor_mode(SensorMode::Normal),
        )
        .await
        .unwrap();
    Timer::after(Duration::from_millis(10)).await;

    loop {
        match sub.next_message_pure().await {
            ModelOutputs::TempRequest(true) => {
                let measurements = bme280.read_sample().await.unwrap();
                let temperature = measurements.temperature.unwrap();
                IN_CHANNEL
                    .send(ModelInputs::TempReading(temperature as f64))
                    .await;
            }
            ModelOutputs::HumRequest(true) => {
                let measurements = bme280.read_sample().await.unwrap();
                let humidity = measurements.humidity.unwrap();
                IN_CHANNEL
                    .send(ModelInputs::HumReading(humidity as f64))
                    .await;
            }
            _ => {}
        }
    }
}

#[embassy_executor::task]
// Task that changes the LED state
async fn led_task(mut led: Output<'static>) {
    // Subscribe to output channel
    let mut sub = OUT_CHANNEL.subscriber().unwrap();
    loop {
        match sub.next_message_pure().await {
            ModelOutputs::LedCommand(state) => {
                if state {
                    led.set_high();
                } else {
                    led.set_low();
                }
                IN_CHANNEL
                    .send(ModelInputs::LedConfirmation(led.is_set_high()))
                    .await;
            }
            _ => {}
        }
    }
}

#[embassy_executor::task]
async fn report_task(mut tx: UartTx<'static, Async>) {
    let mut sub = OUT_CHANNEL.subscriber().unwrap();
    loop {
        match sub.next_message_pure().await {
            ModelOutputs::TempReport((temp, t_sim)) => {
                let mut msg: String<64> = String::new();
                write!(
                    msg,
                    "Temperature;{:.2};{:.2}\r\n",
                    temp,
                    t_sim * 1_000_000.0
                )
                .unwrap();
                embedded_io_async::Write::write(&mut tx, msg.as_bytes())
                    .await
                    .unwrap();
            }
            ModelOutputs::HumReport((hum, t_sim)) => {
                let mut msg: String<64> = String::new();
                write!(msg, "Humidity;{:.2};{:.2}\r\n", hum, t_sim * 1_000_000.0).unwrap();
                embedded_io_async::Write::write(&mut tx, msg.as_bytes())
                    .await
                    .unwrap();
            }
            ModelOutputs::LedReport((state, t_sim)) => {
                let state_str = if state { "ON" } else { "OFF" };
                let mut msg: String<64> = String::new();
                write!(msg, "LED;{};{:.2}\r\n", state_str, t_sim * 1_000_000.0).unwrap();
                embedded_io_async::Write::write(&mut tx, msg.as_bytes())
                    .await
                    .unwrap();
            }
            _ => {}
        }
    }
}

struct Esp32InputHandler {
    last_rt: Option<Instant>,
}

impl Esp32InputHandler {
    fn new() -> Self {
        Self { last_rt: None }
    }
}

impl AsyncInput for Esp32InputHandler {
    type Input = CommonLogicInput;

    async fn handle(
        &mut self,
        config: &Config,
        t_from: f64,
        t_until: f64,
        input: &mut Self::Input,
    ) -> f64 {
        let last_rt = self.last_rt.unwrap_or_else(Instant::now);
        let time_duration = (t_until - t_from) * config.time_scale;
        let time_duration = (time_duration * 1_000_000_000.0) as u64;
        let next_rt = last_rt + Duration::from_nanos(time_duration);

        let future = async {
            // Wait for at least one input
            let rcv = IN_CHANNEL.receiver().receive().await;
            match rcv {
                ModelInputs::Command(cmd) => input.in_command.add_value(cmd).unwrap(),
                ModelInputs::TempReading(temp) => input.in_temp_reading.add_value(temp).unwrap(),
                ModelInputs::HumReading(hum) => input.in_hum_reading.add_value(hum).unwrap(),
                ModelInputs::LedConfirmation(state) => input.in_led_reading.add_value(state).unwrap(),
            };
            // Drain all additional inputs that arrived at the same time
            while let Ok(rcv) = IN_CHANNEL.try_receive() {
                match rcv {
                    ModelInputs::Command(cmd) => input.in_command.add_value(cmd).unwrap(),
                    ModelInputs::TempReading(temp) => input.in_temp_reading.add_value(temp).unwrap(),
                    ModelInputs::HumReading(hum) => input.in_hum_reading.add_value(hum).unwrap(),
                    ModelInputs::LedConfirmation(state) => input.in_led_reading.add_value(state).unwrap(),
                };
            }
        };
        if let Err(_) = with_deadline(next_rt.into(), future).await {
            // Deadline reached (timeout), check for jitter
            if let Some(max_jitter) = config.max_jitter {
                let jitter = Instant::now().duration_since(next_rt);
                let max_jitter_ticks = Duration::from_micros(max_jitter.as_micros() as u64);
                if jitter > max_jitter_ticks {
                    panic!("Jitter too high: {:?}", jitter);
                }
            }
            self.last_rt = Some(next_rt);
            return t_until;
        } else {
            let now = Instant::now();
            self.last_rt = Some(now);
            let elapsed_rt = now.duration_since(last_rt).as_micros() as f64 / 1_000_000.0;
            let elapsed_sim = elapsed_rt / config.time_scale;

            return t_from + elapsed_sim;
        }
    }
}

fn propagate_output(output: &CommonLogicOutput) {
    if let Some(&value) = output.out_temp_req.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::TempRequest(value));
    }
    if let Some(&value) = output.out_hum_req.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::HumRequest(value));
    }
    if let Some(&value) = output.out_led_cmd.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::LedCommand(value));
    }
    if let Some(&(temp, t_sim)) = output.out_temp_rep.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::TempReport((temp, t_sim)));
    }
    if let Some(&(hum, t_sim)) = output.out_hum_rep.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::HumReport((hum, t_sim)));
    }
    if let Some(&(state, t_sim)) = output.out_led_rep.get_values().last() {
        OUT_CHANNEL
            .immediate_publisher()
            .publish_immediate(ModelOutputs::LedReport((state, t_sim)));
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Initial Setup
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let p = esp_hal::init(config);
    let timg0 = TimerGroup::new(p.TIMG0);
    let sw_interrupt = esp_hal::interrupt::software::SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // LED, I2C and UART Setup
    let led = Output::new(p.GPIO9, Level::Low, OutputConfig::default());

    let sda = p.GPIO6;
    let scl = p.GPIO7;

    let i2c = i2c::master::I2c::new(
        p.I2C0,
        i2c::master::Config::default().with_frequency(esp_hal::time::Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(sda)
    .with_scl(scl)
    .into_async();

    let (tx_pin, rx_pin) = (p.GPIO16, p.GPIO17);
    let config = esp_hal::uart::Config::default()
        .with_rx(esp_hal::uart::RxConfig::default().with_fifo_full_threshold(READ_BUF_SIZE as u16));

    let uart0 = esp_hal::uart::Uart::new(p.UART0, config)
        .unwrap()
        .with_tx(tx_pin)
        .with_rx(rx_pin)
        .into_async();

    let (rx, tx) = uart0.split();

    // Prepare simulation
    let controller = common_logic::CommonLogic::new(2.0, 1.0, led.is_set_high());
    let mut simulator = xdevs::simulator::Simulator::new(controller);
    let config = Config::new(0.0, 600.0, 1.0, None);
    let input_handler = Esp32InputHandler::new();

    // Spawn tasks
    spawner.spawn(read_task(rx)).unwrap();
    spawner.spawn(sensor_task(i2c)).unwrap();
    spawner.spawn(led_task(led)).unwrap();
    spawner.spawn(report_task(tx)).unwrap();

    // Run the main simulation task
    simulator
        .simulate_rt_async(&config, input_handler, propagate_output)
        .await;

    esp_println::println!("Simulation completed.");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
