use common_logic::{Command, CommonLogic};
use csv;
use std::fs::File;
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use xdevs::{
    port::Port,
    simulator::{Config, Simulator},
    traits::AsyncInput,
};

mod sim_models;

// Temperature sensor constants
const TEMP_VALUE_MEAN: f64 = 23.354;
const TEMP_VALUE_STD: f64 = 0.0105;
const TEMP_TIME_MEAN: f64 = 1782.484;
const TEMP_TIME_STD: f64 = 134.146;
const TEMP_SEED: u64 = 91827364502;

// Humidity sensor constants
const HUM_VALUE_MEAN: f64 = 37.906;
const HUM_VALUE_STD: f64 = 0.254;
const HUM_TIME_MEAN: f64 = 1778.452;
const HUM_TIME_STD: f64 = 135.135;
const HUM_SEED: u64 = 42583749201;

// LED constants
const LED_TIME_MEAN: f64 = 438.667;
const LED_TIME_STD: f64 = 120.194;
const LED_SEED: u64 = 73619482057;

#[xdevs::coupled(
    couplings = {
        command -> controller.command,
        controller.temp_request -> temp_sensor.trigger,
        controller.hum_request -> hum_sensor.trigger,
        controller.led_command -> led_actuator.command,
        temp_sensor.reading -> controller.temp_reading,
        hum_sensor.reading -> controller.hum_reading,
        led_actuator.state_out -> controller.led_reading,
        controller.temp_report -> reporter.temperature_report,
        controller.hum_report -> reporter.humidity_report,
        controller.led_report -> reporter.led_report,
    }
)]
struct PCModel {
    #[input]
    command: Port<Command, 1>,
    #[components]
    temp_sensor: sim_models::SensorModel,
    hum_sensor: sim_models::SensorModel,
    led_actuator: sim_models::LedModel,
    reporter: sim_models::ReportModel,
    controller: CommonLogic,
}

struct InputHandler {
    sender: Sender<Command>,
    receiver: Receiver<Command>,
    last_rt: Option<Instant>,
}

impl InputHandler {
    fn new(buffer: usize) -> Self {
        let (sender, receiver) = channel(buffer);
        Self {
            sender,
            receiver,
            last_rt: None,
        }
    }
}

impl AsyncInput for InputHandler {
    type Input = PCModelInput;

    async fn handle(
        &mut self,
        config: &Config,
        t_from: f64,
        t_until: f64,
        input: &mut Self::Input,
    ) -> f64 {
        let last_rt = self.last_rt.unwrap_or_else(Instant::now);
        let next_rt = last_rt + Duration::from_secs_f64((t_until - t_from) * config.time_scale);

        let future = async {
            let cmd = self.receiver.recv().await;
            if let Some(cmd) = cmd {
                input.command.add_value(cmd).unwrap();
            }
        };
        if let Err(_) = tokio::time::timeout_at(next_rt.into(), future).await {
            self.last_rt = Some(next_rt);
            return t_until;
        } else {
            let now = Instant::now();
            self.last_rt = Some(now);
            let elapsed_rt = now.duration_since(last_rt).as_secs_f64();
            let elapsed_sim = elapsed_rt / config.time_scale;

            return t_from + elapsed_sim;
        }
    }
}

/// Asynchronous function to handle reading commands from standard input.
async fn input_reader_task(sender: Sender<Command>) {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    println!("Simulation started. Type commands below:");

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();

        let command = match trimmed {
            "TEMP ON" => Some(Command::TempOn),
            "TEMP OFF" => Some(Command::TempOff),
            "HUM ON" => Some(Command::HumOn),
            "HUM OFF" => Some(Command::HumOff),
            "LED ON" => Some(Command::LedOn),
            "LED OFF" => Some(Command::LedOff),
            _ => {
                println!("Unknown command: {}", trimmed);
                None
            }
        };

        if let Some(cmd) = command {
            sender.send(cmd).await.unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize CSV writer
    let file = File::create("data.csv").unwrap();
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(&["Type", "Value", "Time"]).unwrap();

    // Initialize simulation models
    let temp_sensor = sim_models::SensorModel::create(
        TEMP_VALUE_MEAN,
        TEMP_VALUE_STD,
        TEMP_TIME_MEAN,
        TEMP_TIME_STD,
        TEMP_SEED,
    );
    let hum_sensor = sim_models::SensorModel::create(
        HUM_VALUE_MEAN,
        HUM_VALUE_STD,
        HUM_TIME_MEAN,
        HUM_TIME_STD,
        HUM_SEED,
    );
    let led_actuator = sim_models::LedModel::create(LED_TIME_MEAN, LED_TIME_STD, LED_SEED);
    let reporter = sim_models::ReportModel::new(wtr, f64::INFINITY);
    let controller = CommonLogic::create(2.0, 1.0, false);
    let pc_sim = PCModel::new(
        temp_sensor,
        hum_sensor,
        led_actuator,
        reporter,
        controller,
    );

    // Set up the simulator and input handler
    let mut simulator = Simulator::new(pc_sim);
    let config = Config::new(0.0, 600.0, 1.0, None);
    let input_handler = InputHandler::new(20);
    let sender = input_handler.sender.clone();

    // Spawn the dedicated input reader function
    tokio::spawn(input_reader_task(sender));

    // Run the main simulation task
    simulator
        .simulate_rt_async(&config, input_handler, |_| {})
        .await;
    println!("Simulation completed.");
}
