use csv;
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use std::fs::File;
use xdevs::port::Port;

#[xdevs::atomic]
pub struct TemperatureSensorModel {
    #[input]
    get_temp: Port<bool, 1>,
    #[output]
    temp_out: Port<f64, 1>,
    #[state]
    temp: Option<f64>,
    temp_dist: Normal<f64>,
    time_dist: Normal<f64>,
    rng: StdRng,
    sigma: f64,
}

impl xdevs::Atomic for TemperatureSensorModel {
    fn delta_int(state: &mut Self::State) {
        state.temp = None;
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(temp) = state.temp {
            output.temp_out.add_value(temp).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(_) = input.get_temp.get_values().last() {
            state.temp = Some(state.temp_dist.sample(&mut state.rng));
            state.sigma = state.time_dist.sample(&mut state.rng) / 1_000_000.0;
        }
    }
}

impl TemperatureSensorModel {
    pub fn start() -> Self {
        const TEMP_VALUE_MEAN: f64 = 23.354;
        const TEMP_VALUE_STD: f64 = 0.0105;
        const TEMP_TIME_MEAN: f64 = 1782.484;
        const TEMP_TIME_STD: f64 = 134.146;
        const TEMP_SEED: u64 = 91827364502;

        let temp_dist = Normal::new(TEMP_VALUE_MEAN, TEMP_VALUE_STD).unwrap();
        let time_dist = Normal::new(TEMP_TIME_MEAN, TEMP_TIME_STD).unwrap();
        let rng = StdRng::seed_from_u64(TEMP_SEED);
        TemperatureSensorModel::new(None, temp_dist, time_dist, rng, f64::INFINITY)
    }
}

#[xdevs::atomic]
pub struct HumiditySensorModel {
    #[input]
    get_hum: Port<bool, 1>,
    #[output]
    hum_out: Port<f64, 1>,
    #[state]
    hum: Option<f64>,
    hum_dist: Normal<f64>,
    time_dist: Normal<f64>,
    rng: StdRng,
    sigma: f64,
}

impl xdevs::Atomic for HumiditySensorModel {
    fn delta_int(state: &mut Self::State) {
        state.hum = None;
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(hum) = state.hum {
            output.hum_out.add_value(hum).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(_) = input.get_hum.get_values().last() {
            state.hum = Some(state.hum_dist.sample(&mut state.rng));
            state.sigma = state.time_dist.sample(&mut state.rng) / 1_000_000.0;
        }
    }
}

impl HumiditySensorModel {
    pub fn start() -> Self {
        const HUM_VALUE_MEAN: f64 = 37.906;
        const HUM_VALUE_STD: f64 = 0.254;
        const HUM_TIME_MEAN: f64 = 1778.452;
        const HUM_TIME_STD: f64 = 135.135;
        const HUM_SEED: u64 = 42583749201;

        let hum_dist = Normal::new(HUM_VALUE_MEAN, HUM_VALUE_STD).unwrap();
        let time_dist = Normal::new(HUM_TIME_MEAN, HUM_TIME_STD).unwrap();
        let rng = StdRng::seed_from_u64(HUM_SEED);
        HumiditySensorModel::new(None, hum_dist, time_dist, rng, f64::INFINITY)
    }
}

#[xdevs::atomic]
pub struct LedSensorModel {
    #[input]
    led_cmd: Port<bool, 1>,
    #[output]
    led_out: Port<bool, 1>,
    #[state]
    led: Option<bool>,
    time_dist: Normal<f64>,
    rng: StdRng,
    sigma: f64,
}

impl xdevs::Atomic for LedSensorModel {
    fn delta_int(state: &mut Self::State) {
        state.led = None;
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(led) = state.led {
            output.led_out.add_value(led).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(&led_cmd) = input.led_cmd.get_values().last() {
            state.led = Some(led_cmd);
            state.sigma = state.time_dist.sample(&mut state.rng) / 1_000_000.0;
        }
    }
}

impl LedSensorModel {
    pub fn start() -> Self {
        const LED_TIME_MEAN: f64 = 438.667;
        const LED_TIME_STD: f64 = 120.194;
        const LED_SEED: u64 = 73619482057;

        let time_dist = Normal::new(LED_TIME_MEAN, LED_TIME_STD).unwrap();
        let rng = StdRng::seed_from_u64(LED_SEED);
        LedSensorModel::new(None, time_dist, rng, f64::INFINITY)
    }
}

#[xdevs::atomic]
struct ReportModel {
    #[input]
    temp_report: Port<(f64, f64), 1>,
    hum_report: Port<(f64, f64), 1>,
    led_report: Port<(bool, f64), 1>,
    #[state]
    wtr: csv::Writer<File>,
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
        if let Some(&(temp, time)) = input.temp_report.get_values().last() {
            let time = time * 1000_000.0;
            state
                .wtr
                .write_record(&[
                    "Temperature",
                    &format!("{:.2}", temp),
                    &format!("{:.2}", time),
                ])
                .unwrap();
            state.wtr.flush().unwrap();
        }
        if let Some(&(hum, time)) = input.hum_report.get_values().last() {
            let time = time * 1000_000.0;
            state
                .wtr
                .write_record(&["Humidity", &format!("{:.2}", hum), &format!("{:.2}", time)])
                .unwrap();
            state.wtr.flush().unwrap();
        }
        if let Some(&(led, time)) = input.led_report.get_values().last() {
            let time = time * 1000_000.0;
            let led_state = if led { "ON" } else { "OFF" };
            state
                .wtr
                .write_record(&["LED", &led_state.to_string(), &format!("{:.2}", time)])
                .unwrap();
            state.wtr.flush().unwrap();
            println!("LED state: {}", led);
        }
        state.sigma = f64::INFINITY;
    }
}
