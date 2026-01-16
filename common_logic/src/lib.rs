#![no_std]

use xdevs::port::Port;

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

#[xdevs::atomic]
struct Orchestrator {
    #[input]
    command: Port<Command, 1>,
    #[output]
    temp_enable: Port<bool, 1>,
    hum_enable: Port<bool, 1>,
    led_set: Port<bool, 1>,
    #[state]
    pending_command: Option<Command>,
    sigma: f64,
}

impl xdevs::Atomic for Orchestrator {
    fn delta_int(state: &mut Self::State) {
        state.sigma = f64::INFINITY;
    }

    fn lambda(state: &Self::State, output: &mut Self::Output) {
        if let Some(command) = state.pending_command {
            match command {
                Command::TempOn => output.temp_enable.add_value(true).unwrap(),
                Command::TempOff => output.temp_enable.add_value(false).unwrap(),
                Command::HumOn => output.hum_enable.add_value(true).unwrap(),
                Command::HumOff => output.hum_enable.add_value(false).unwrap(),
                Command::LedOn => output.led_set.add_value(true).unwrap(),
                Command::LedOff => output.led_set.add_value(false).unwrap(),
            }
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(command) = input.command.get_values().last() {
            state.pending_command = Some(*command);
            state.sigma = 0.0;
        }
    }
}

impl Orchestrator {
    pub fn create() -> Self {
        Orchestrator::new(None, f64::INFINITY)
    }
}

#[xdevs::atomic]
struct SensorHandler {
    #[input]
    enable: Port<bool, 1>,
    reading: Port<f64, 1>,
    #[output]
    request: Port<bool, 1>,
    report: Port<(f64, f64), 1>,
    #[state]
    phase: Mode,
    sigma: f64,
    pending_report: (f64, f64),
    period: f64,
    deadline: f64,
}

impl xdevs::Atomic for SensorHandler {
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
                output.request.add_value(true).unwrap();
            }
            Mode::AckReceived => {
                output.report.add_value(state.pending_report).unwrap();
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
                if let Some(&enabled) = input.enable.get_values().last() {
                    if enabled {
                        state.phase = Mode::Idle;
                        state.sigma = 0.0;
                    }
                }
            }
            Mode::Idle => {
                if let Some(&enabled) = input.enable.get_values().last() {
                    if !enabled {
                        state.phase = Mode::Off;
                        state.sigma = f64::INFINITY;
                    }
                }
            }
            Mode::WaitingAck => {
                if let Some(&enabled) = input.enable.get_values().last() {
                    if !enabled {
                        state.phase = Mode::Off;
                        state.sigma = f64::INFINITY;
                    }
                } else if let Some(&value) = input.reading.get_values().last() {
                    state.pending_report = (value, elapsed);
                    state.phase = Mode::AckReceived;
                    state.sigma = state.period - elapsed;
                }
            }
            Mode::AckReceived => {
                if let Some(&enabled) = input.enable.get_values().last() {
                    if !enabled {
                        state.phase = Mode::Off;
                        state.sigma = f64::INFINITY;
                    }
                }
            }
        }
    }
}

impl SensorHandler {
    pub fn create(period: f64, deadline: f64) -> Self {
        SensorHandler::new(Mode::Off, f64::INFINITY, (0.0, 0.0), period, deadline)
    }
}

#[xdevs::atomic]
struct LedHandler {
    #[input]
    enable: Port<bool, 1>,
    confirmation: Port<bool, 1>,
    #[output]
    set_state: Port<bool, 1>,
    report: Port<(bool, f64), 1>,
    #[state]
    phase: Mode,
    sigma: f64,
    pending_report: (bool, f64),
    target_state: bool,
    deadline: f64,
}

impl xdevs::Atomic for LedHandler {
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
                output.set_state.add_value(state.target_state).unwrap();
            }
            Mode::AckReceived => {
                output.report.add_value(state.pending_report).unwrap();
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
                if let Some(&new_state) = input.enable.get_values().last() {
                    state.target_state = new_state;
                    state.phase = Mode::Idle;
                    state.sigma = 0.0;
                }
            }
            Mode::WaitingAck => {
                if let Some(&confirmed) = input.confirmation.get_values().last() {
                    state.pending_report = (confirmed, elapsed);
                    state.phase = Mode::AckReceived;
                    state.sigma = 0.0;
                }
            }
            _ => {}
        }
    }
}

impl LedHandler {
    pub fn create(initial_state: bool, deadline: f64) -> Self {
        LedHandler::new(Mode::Off, f64::INFINITY, (false, 0.0), initial_state, deadline)
    }
}

#[xdevs::coupled(
couplings = {
        command -> orchestrator.command,
        temp_reading -> temp_handler.reading,
        hum_reading -> hum_handler.reading,
        led_reading -> led_handler.confirmation,
        orchestrator.temp_enable -> temp_handler.enable,
        orchestrator.hum_enable -> hum_handler.enable,
        orchestrator.led_set -> led_handler.enable,
        temp_handler.request -> temp_request,
        temp_handler.report -> temp_report,
        hum_handler.request -> hum_request,
        hum_handler.report -> hum_report,
        led_handler.set_state -> led_command,
        led_handler.report -> led_report,
    }
)]
pub struct CommonLogic {
    #[input]
    command: Port<Command, 1>,
    temp_reading: Port<f64, 1>,
    hum_reading: Port<f64, 1>,
    led_reading: Port<bool, 1>,
    #[output]
    temp_request: Port<bool, 1>,
    hum_request: Port<bool, 1>,
    led_command: Port<bool, 1>,
    temp_report: Port<(f64, f64), 1>,
    hum_report: Port<(f64, f64), 1>,
    led_report: Port<(bool, f64), 1>,
    #[components]
    orchestrator: Orchestrator,
    temp_handler: SensorHandler,
    hum_handler: SensorHandler,
    led_handler: LedHandler,
}

impl CommonLogic {
    pub fn create(period: f64, deadline: f64, initial_led_state: bool) -> Self {
        CommonLogic::new(
            Orchestrator::create(),
            SensorHandler::create(period, deadline),
            SensorHandler::create(period, deadline),
            LedHandler::create(initial_led_state, deadline),
        )
    }
}

