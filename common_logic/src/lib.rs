#![no_std]

#[derive(Copy, Clone, Debug)]
pub enum Command {
    TempOn,
    TempOff,
    HumOn,
    HumOff,
    LedOn,
    LedOff,
}

#[derive(Copy, Clone, Debug)]
enum Mode {
    Off,
    Idle,
    WaitingAck,
    AckReceived,
}

pub mod orchestrator {
    use super::Command;
    use xdevs::port::Port;

    #[xdevs::atomic]
    struct Orchestrator {
        #[input]
        command: Port<Command, 1>,
        #[output]
        temp_toggle: Port<bool, 1>,
        hum_toggle: Port<bool, 1>,
        led_cmd: Port<bool, 1>,
        #[state]
        command: Option<Command>,
        sigma: f64,
    }

    impl xdevs::Atomic for Orchestrator {
        fn delta_int(state: &mut Self::State) {
            state.sigma = f64::INFINITY;
        }

        fn lambda(state: &Self::State, output: &mut Self::Output) {
            if let Some(command) = state.command {
                match command {
                    Command::TempOn => output.temp_toggle.add_value(true).unwrap(),
                    Command::TempOff => output.temp_toggle.add_value(false).unwrap(),
                    Command::HumOn => output.hum_toggle.add_value(true).unwrap(),
                    Command::HumOff => output.hum_toggle.add_value(false).unwrap(),
                    Command::LedOn => output.led_cmd.add_value(true).unwrap(),
                    Command::LedOff => output.led_cmd.add_value(false).unwrap(),
                }
            }
        }

        fn ta(state: &Self::State) -> f64 {
            state.sigma
        }

        fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
            if let Some(command) = input.command.get_values().last() {
                state.command = Some(*command);
                state.sigma = 0.0;
            }
        }
    }

    impl Orchestrator {
        pub fn start() -> Self {
            Orchestrator::new(None, f64::INFINITY)
        }
    }
}

pub mod measure {
    use super::Mode;
    use xdevs::port::Port;

    #[xdevs::atomic]
    struct Measure {
        #[input]
        toggle: Port<bool, 1>,
        ack: Port<f64, 1>,
        #[output]
        get_meas: Port<bool, 1>,
        report: Port<(f64, f64), 1>,
        #[state]
        phase: Mode,
        sigma: f64,
        report: (f64, f64),
        period: f64,
        deadline: f64,
    }

    impl xdevs::Atomic for Measure {
        fn delta_int(state: &mut Self::State) {
            match state.phase {
                Mode::Idle => {
                    state.phase = Mode::WaitingAck;
                    state.sigma = state.deadline;
                }
                Mode::WaitingAck => {
                    state.phase = Mode::Idle;
                    state.sigma = state.period - state.deadline;
                }
                Mode::AckReceived => {
                    state.phase = Mode::Idle;
                    state.sigma = 0.0;
                }
                _ => {}
            }
        }

        fn lambda(state: &Self::State, output: &mut Self::Output) {
            match state.phase {
                Mode::Idle => {
                    output.get_meas.add_value(true).unwrap();
                }
                Mode::AckReceived => {
                    output.report.add_value(state.report).unwrap();
                }
                _ => {}
            }
        }

        fn ta(state: &Self::State) -> f64 {
            state.sigma
        }

        fn delta_ext(state: &mut Self::State, elapsed: f64, input: &Self::Input) {
            match state.phase {
                Mode::Off => {
                    if let Some(&toggle) = input.toggle.get_values().last() {
                        if toggle {
                            state.phase = Mode::Idle;
                            state.sigma = 0.0;
                        }
                    }
                }
                Mode::Idle => {
                    if let Some(&toggle) = input.toggle.get_values().last() {
                        if !toggle {
                            state.phase = Mode::Off;
                            state.sigma = f64::INFINITY;
                        }
                    }
                }
                Mode::WaitingAck => {
                    if let Some(&toggle) = input.toggle.get_values().last() {
                        if !toggle {
                            state.phase = Mode::Off;
                            state.sigma = f64::INFINITY;
                        }
                    } else if let Some(&ack) = input.ack.get_values().last() {
                        state.report = (ack, elapsed);
                        state.phase = Mode::AckReceived;
                        state.sigma = state.period - elapsed;
                    }
                }
                Mode::AckReceived => {
                    if let Some(&toggle) = input.toggle.get_values().last() {
                        if !toggle {
                            state.phase = Mode::Off;
                            state.sigma = f64::INFINITY;
                        }
                    }
                }
            }
        }
    }

    impl Measure {
        pub fn start(period: f64, deadline: f64) -> Self {
            Measure::new(Mode::Off, f64::INFINITY, (0.0, 0.0), period, deadline)
        }
    }
}

pub mod led_control {
    use super::Mode;
    use xdevs::port::Port;

    #[xdevs::atomic]
    struct LedControl {
        #[input]
        led_toggle: Port<bool, 1>,
        led_ack: Port<bool, 1>,
        #[output]
        led_cmd: Port<bool, 1>,
        led_report: Port<(bool, f64), 1>,
        #[state]
        phase: Mode,
        sigma: f64,
        report: (bool, f64),
        led_on: bool,
        deadline: f64,
    }

    impl xdevs::Atomic for LedControl {
        fn delta_int(state: &mut Self::State) {
            match state.phase {
                Mode::Idle => {
                    state.phase = Mode::WaitingAck;
                    state.sigma = state.deadline;
                }
                Mode::AckReceived => {
                    state.phase = Mode::Off;
                    state.sigma = f64::INFINITY;
                }
                _ => {}
            }
        }

        fn lambda(state: &Self::State, output: &mut Self::Output) {
            match state.phase {
                Mode::Idle => {
                    output.led_cmd.add_value(state.led_on).unwrap();
                }
                Mode::AckReceived => {
                    output.led_report.add_value(state.report).unwrap();
                }
                _ => {}
            }
        }

        fn ta(state: &Self::State) -> f64 {
            state.sigma
        }

        fn delta_ext(state: &mut Self::State, elapsed: f64, input: &Self::Input) {
            match state.phase {
                Mode::Off => {
                    if let Some(&toggle) = input.led_toggle.get_values().last() {
                        state.led_on = toggle;
                        state.phase = Mode::Idle;
                        state.sigma = 0.0;
                    }
                }
                Mode::WaitingAck => {
                    if let Some(&ack) = input.led_ack.get_values().last() {
                        state.report = (ack, elapsed);
                        state.phase = Mode::AckReceived;
                        state.sigma = 0.0;
                    }
                }
                _ => {}
            }
        }
    }

    impl LedControl {
        pub fn start(led_on: bool, deadline: f64) -> Self {
            LedControl::new(Mode::Off, f64::INFINITY, (false, 0.0), led_on, deadline)
        }
    }
}

#[xdevs::coupled(
couplings = {
        command -> orchestrator.command,
        temp_ack -> temp_measure.ack,
        hum_ack -> hum_measure.ack,
        led_ack -> led_control.led_ack,
        orchestrator.temp_toggle -> temp_measure.toggle,
        orchestrator.hum_toggle -> hum_measure.toggle,
        orchestrator.led_cmd -> led_control.led_toggle,
        temp_measure.get_meas -> get_temp,
        temp_measure.report -> temp_report,
        hum_measure.get_meas -> get_hum,
        hum_measure.report -> hum_report,
        led_control.led_cmd -> led_cmd,
        led_control.led_report -> led_report,
    }
)]
pub struct ProcessorModel {
    #[input]
    command: xdevs::port::Port<Command, 1>,
    temp_ack: xdevs::port::Port<f64, 1>,
    hum_ack: xdevs::port::Port<f64, 1>,
    led_ack: xdevs::port::Port<bool, 1>,
    #[output]
    get_temp: xdevs::port::Port<bool, 1>,
    get_hum: xdevs::port::Port<bool, 1>,
    led_cmd: xdevs::port::Port<bool, 1>,
    temp_report: xdevs::port::Port<(f64, f64), 1>,
    hum_report: xdevs::port::Port<(f64, f64), 1>,
    led_report: xdevs::port::Port<(bool, f64), 1>,
    #[components]
    orchestrator: orchestrator::Orchestrator,
    temp_measure: measure::Measure,
    hum_measure: measure::Measure,
    led_control: led_control::LedControl,
}

impl ProcessorModel {
    pub fn start(period: f64, deadline: f64, led_on: bool) -> Self {
        ProcessorModel::new(
            orchestrator::Orchestrator::start(),
            measure::Measure::start(period, deadline),
            measure::Measure::start(period, deadline),
            led_control::LedControl::start(led_on, deadline),
        )
    }
}
