use csv;
use std::fs::File;
use xdevs::port::Port;

#[xdevs::atomic]
pub struct SensorModel {
    #[input]
    get_temp: Port<bool, 1>,
    get_hum: Port<bool, 1>,
    led_cmd: Port<bool, 1>,
    #[output]
    temp_out: Port<f64, 1>,
    hum_out: Port<f64, 1>,
    led_out: Port<bool, 1>,
    #[state]
    temp: Option<f64>,
    hum: Option<f64>,
    led: Option<bool>,
    sigma: f64,
}

impl xdevs::Atomic for SensorModel {
    fn delta_int(state: &mut Self::State) {
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(temp) = state.temp {
            output.temp_out.add_value(temp).unwrap();
        }
        if let Some(hum) = state.hum {
            output.hum_out.add_value(hum).unwrap();
        }
        if let Some(led) = state.led {
            output.led_out.add_value(led).unwrap();
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(_) = input.get_temp.get_values().last() {
            state.temp = Some(25.0);
        } else {
            state.temp = None;
        }
        if let Some(_) = input.get_hum.get_values().last() {
            state.hum = Some(60.0);
        } else {
            state.hum = None;
        }
        if let Some(&led_cmd) = input.led_cmd.get_values().last() {
            state.led = Some(led_cmd);
        } else {
            state.led = None;
        }
        state.sigma = 0.0;
    }
}

impl SensorModel {
    pub fn start() -> Self {
        SensorModel::new(None, None, None, f64::INFINITY)
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
            state
                .wtr
                .write_record(&["Temperature", &temp.to_string(), &time.to_string()])
                .unwrap();
            state.wtr.flush().unwrap();
        }
        if let Some(&(hum, time)) = input.hum_report.get_values().last() {
            state
                .wtr
                .write_record(&["Humidity", &hum.to_string(), &time.to_string()])
                .unwrap();
            state.wtr.flush().unwrap();
        }
        if let Some(&(led, time)) = input.led_report.get_values().last() {
            state
                .wtr
                .write_record(&["LED", &led.to_string(), &time.to_string()])
                .unwrap();
            state.wtr.flush().unwrap();
            println!("LED state: {}", led);
        }
        state.sigma = f64::INFINITY;
    }
}
