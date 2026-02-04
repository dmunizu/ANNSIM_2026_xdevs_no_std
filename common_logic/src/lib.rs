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
    in_command: Port<Command, 1>,
    #[output]
    out_temp_enable: Port<bool, 1>,
    out_hum_enable: Port<bool, 1>,
    out_led_set: Port<bool, 1>,
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
                Command::TempOn => output.out_temp_enable.add_value(true).unwrap(),
                Command::TempOff => output.out_temp_enable.add_value(false).unwrap(),
                Command::HumOn => output.out_hum_enable.add_value(true).unwrap(),
                Command::HumOff => output.out_hum_enable.add_value(false).unwrap(),
                Command::LedOn => output.out_led_set.add_value(true).unwrap(),
                Command::LedOff => output.out_led_set.add_value(false).unwrap(),
            }
        }
    }

    fn ta(state: &Self::State) -> f64 {
        state.sigma
    }

    fn delta_ext(state: &mut Self::State, _elapsed: f64, input: &Self::Input) {
        if let Some(command) = input.in_command.get_values().last() {
            state.pending_command = Some(*command);
            state.sigma = 0.0;
        }
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        Orchestrator::build(None, f64::INFINITY)
    }
}

#[xdevs::atomic]
struct SensorHandler {
    #[input]
    in_enable: Port<bool, 1>,
    in_reading: Port<f64, 1>,
    #[output]
    out_request: Port<bool, 1>,
    out_report: Port<(f64, f64), 1>,
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
                output.out_request.add_value(true).unwrap();
            }
            Mode::AckReceived => {
                output.out_report.add_value(state.pending_report).unwrap();
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
                if let Some(&enabled) = input.in_enable.get_values().last() {
                    if enabled {
                        state.phase = Mode::Idle;
                        state.sigma = 0.0;
                    }
                }
            }
            Mode::Idle => {
                if let Some(&enabled) = input.in_enable.get_values().last() {
                    if !enabled {
                        state.phase = Mode::Off;
                        state.sigma = f64::INFINITY;
                    }
                }
            }
            Mode::WaitingAck => {
                if let Some(&enabled) = input.in_enable.get_values().last() {
                    if !enabled {
                        state.phase = Mode::Off;
                        state.sigma = f64::INFINITY;
                    }
                } else if let Some(&value) = input.in_reading.get_values().last() {
                    state.pending_report = (value, elapsed);
                    state.phase = Mode::AckReceived;
                    state.sigma = state.period - elapsed;
                }
            }
            Mode::AckReceived => {
                if let Some(&enabled) = input.in_enable.get_values().last() {
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
    pub fn new(period: f64, deadline: f64) -> Self {
        SensorHandler::build(Mode::Off, f64::INFINITY, (0.0, 0.0), period, deadline)
    }
}

#[xdevs::atomic]
struct LedHandler {
    #[input]
    in_enable: Port<bool, 1>,
    in_confirmation: Port<bool, 1>,
    #[output]
    out_set_state: Port<bool, 1>,
    out_report: Port<(bool, f64), 1>,
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
                output.out_set_state.add_value(state.target_state).unwrap();
            }
            Mode::AckReceived => {
                output.out_report.add_value(state.pending_report).unwrap();
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
                if let Some(&new_state) = input.in_enable.get_values().last() {
                    state.target_state = new_state;
                    state.phase = Mode::Idle;
                    state.sigma = 0.0;
                }
            }
            Mode::WaitingAck => {
                if let Some(&confirmed) = input.in_confirmation.get_values().last() {
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
    pub fn new(initial_state: bool, deadline: f64) -> Self {
        LedHandler::build(Mode::Off, f64::INFINITY, (false, 0.0), initial_state, deadline)
    }
}

#[xdevs::coupled(
couplings = {
        in_command -> orchestrator.in_command,
        in_temp_reading -> temp_handler.in_reading,
        in_hum_reading -> hum_handler.in_reading,
        in_led_reading -> led_handler.in_confirmation,
        orchestrator.out_temp_enable -> temp_handler.in_enable,
        orchestrator.out_hum_enable -> hum_handler.in_enable,
        orchestrator.out_led_set -> led_handler.in_enable,
        temp_handler.out_request -> out_temp_req,
        temp_handler.out_report -> out_temp_rep,
        hum_handler.out_request -> out_hum_req,
        hum_handler.out_report -> out_hum_rep,
        led_handler.out_set_state -> out_led_cmd,
        led_handler.out_report -> out_led_rep,
    }
)]
pub struct CommonLogic {
    #[input]
    in_command: Port<Command, 1>,
    in_temp_reading: Port<f64, 1>,
    in_hum_reading: Port<f64, 1>,
    in_led_reading: Port<bool, 1>,
    #[output]
    out_temp_req: Port<bool, 1>,
    out_hum_req: Port<bool, 1>,
    out_led_cmd: Port<bool, 1>,
    out_temp_rep: Port<(f64, f64), 1>,
    out_hum_rep: Port<(f64, f64), 1>,
    out_led_rep: Port<(bool, f64), 1>,
    #[components]
    orchestrator: Orchestrator,
    temp_handler: SensorHandler,
    hum_handler: SensorHandler,
    led_handler: LedHandler,
}

impl CommonLogic {
    pub fn new(period: f64, deadline: f64, initial_led_state: bool) -> Self {
        CommonLogic::build(
            Orchestrator::new(),
            SensorHandler::new(period, deadline),
            SensorHandler::new(period, deadline),
            LedHandler::new(initial_led_state, deadline),
        )
    }
}

