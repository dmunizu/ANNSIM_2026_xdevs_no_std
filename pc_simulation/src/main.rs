use common_logic::{Command, ProcessorModel};
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

#[xdevs::coupled(
    couplings = {
        command -> processor_model.command,
        processor_model.get_temp -> sensor_model.get_temp,
        processor_model.get_hum -> sensor_model.get_hum,
        processor_model.led_cmd -> sensor_model.led_cmd,
        sensor_model.temp_out -> processor_model.temp_ack,
        sensor_model.hum_out -> processor_model.hum_ack,
        sensor_model.led_out -> processor_model.led_ack,
        processor_model.temp_report -> report_model.temp_report,
        processor_model.hum_report -> report_model.hum_report,
        processor_model.led_report -> report_model.led_report,
    }
)]
struct PCSimulation {
    #[input]
    command: Port<Command, 1>,
    #[components]
    sensor_model: sim_models::SensorModel,
    report_model: sim_models::ReportModel,
    processor_model: ProcessorModel,
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
    type Input = PCSimulationInput;

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
    let file = File::create("data.csv").unwrap();
    let mut wtr = csv::WriterBuilder::new().delimiter(b';').from_writer(file);
    wtr.write_record(&["Type", "Value", "Time"]).unwrap();

    let sensor_sim = sim_models::SensorModel::start();
    let report_model = sim_models::ReportModel::new(wtr, f64::INFINITY);
    let processor_model = ProcessorModel::start(2.0, 1.0, false);
    let pc_sim = PCSimulation::new(sensor_sim, report_model, processor_model);

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
