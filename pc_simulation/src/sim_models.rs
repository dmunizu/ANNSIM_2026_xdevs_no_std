use csv;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use std::fs::File;
use xdevs::port::Port;

#[xdevs::atomic]
pub struct SensorModel {
    #[input]
    in_trigger: Port<bool, 1>,
    #[output]
    out_reading: Port<f64, 1>,
    #[state]
    pending_value: Option<f64>,
    value_dist: Normal<f64>,
    time_dist: Normal<f64>,
    rng: StdRng,
    sigma: f64,
}

impl xdevs::Atomic for SensorModel {
    fn delta_int(state: &mut Self::State) {
        state.pending_value = None;
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(value) = state.pending_value {
            output.out_reading.add_value(value).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(_) = input.in_trigger.get_values().last() {
            state.pending_value = Some(state.value_dist.sample(&mut state.rng));
            state.sigma = state.time_dist.sample(&mut state.rng) / 1_000_000.0;
        }
    }
}

impl SensorModel {
    pub fn new(
        value_mean: f64,
        value_std: f64,
        time_mean: f64,
        time_std: f64,
        seed: u64,
    ) -> Self {
        let value_dist = Normal::new(value_mean, value_std).unwrap();
        let time_dist = Normal::new(time_mean, time_std).unwrap();
        let rng = StdRng::seed_from_u64(seed);
        SensorModel::build(None, value_dist, time_dist, rng, f64::INFINITY)
    }
}

#[xdevs::atomic]
pub struct LedModel {
    #[input]
    in_command: Port<bool, 1>,
    #[output]
    out_state: Port<bool, 1>,
    #[state]
    pending_state: Option<bool>,
    time_dist: Normal<f64>,
    rng: StdRng,
    sigma: f64,
}

impl xdevs::Atomic for LedModel {
    fn delta_int(state: &mut Self::State) {
        state.pending_state = None;
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(led_state) = state.pending_state {
            output.out_state.add_value(led_state).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(&cmd) = input.in_command.get_values().last() {
            state.pending_state = Some(cmd);
            state.sigma = state.time_dist.sample(&mut state.rng) / 1_000_000.0;
        }
    }
}

impl LedModel {
    pub fn new(time_mean: f64, time_std: f64, seed: u64) -> Self {
        let time_dist = Normal::new(time_mean, time_std).unwrap();
        let rng = StdRng::seed_from_u64(seed);
        LedModel::build(None, time_dist, rng, f64::INFINITY)
    }
}

#[xdevs::atomic]
struct ReportModel {
    #[input]
    in_temp_rep: Port<(f64, f64), 1>,
    in_hum_rep: Port<(f64, f64), 1>,
    in_led_rep: Port<(bool, f64), 1>,
    #[state]
    writer: csv::Writer<File>,
    sigma: f64,
}
impl xdevs::Atomic for ReportModel {
    fn delta_int(state: &mut Self::State) {
        state.sigma = f64::INFINITY;
    }

    fn lambda(_state: &Self::State, _output: &mut Self::Output) {}

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(&(temp, time)) = input.in_temp_rep.get_values().last() {
            let time = time * 1000_000.0;
            state
                .writer
                .write_record(&[
                    "Temperature",
                    &format!("{:.2}", temp),
                    &format!("{:.2}", time),
                ])
                .unwrap();
            state.writer.flush().unwrap();
        }
        if let Some(&(hum, time)) = input.in_hum_rep.get_values().last() {
            let time = time * 1000_000.0;
            state
                .writer
                .write_record(&["Humidity", &format!("{:.2}", hum), &format!("{:.2}", time)])
                .unwrap();
            state.writer.flush().unwrap();
        }
        if let Some(&(led, time)) = input.in_led_rep.get_values().last() {
            let time = time * 1000_000.0;
            let led_state = if led { "ON" } else { "OFF" };
            state
                .writer
                .write_record(&["LED", &led_state.to_string(), &format!("{:.2}", time)])
                .unwrap();
            state.writer.flush().unwrap();
            println!("LED state: {}", led);
        }
        state.sigma = f64::INFINITY;
    }
}
