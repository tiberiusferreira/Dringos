use pzem004t::pzemv1::Pzem;
use std::ops::Deref;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;

const IDLE_TIME_CONSIDER_DONE: u64 = 7 * 60;

pub struct WashingMachine {
    state: Arc<RwLock<State>>,
    rx: Receiver<Event>,
}

impl WashingMachine {
    pub fn get_state(&self) -> State {
        let read_guard = self.state.read().unwrap();
        read_guard.deref().clone()
    }
    pub fn check_for_events(&self) -> Option<Event> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                panic!("Receiver disconnected!")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    NowAvailable { finished_cycle_stats: InUseCounters },
    NowInUse,
}

impl WashingMachine {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Event>(2);
        let usb_port = "/dev/ttyUSB0";
        let port = serialport::new(usb_port, 9600)
            .timeout(Duration::from_millis(2000))
            .open()
            .unwrap_or_else(|e| panic!("Cannot open `{}`: {}.", usb_port, e));
        let mut pzem = Pzem::new(port);
        let state = Arc::new(RwLock::new(State::Available));
        let thread_state_handle = state.clone();
        std::thread::spawn(move || loop {
            match pzem.read_power() {
                Ok(power) => {
                    let previous_state = thread_state_handle.read().unwrap().deref().clone();
                    let new_state = previous_state.to_new_state(power);
                    let mut write_lock = thread_state_handle.write().unwrap();
                    (*write_lock) = new_state.clone();
                    drop(write_lock);
                    match (previous_state, new_state) {
                        (State::Available, State::InUse(_counters)) => {
                            log::info!("Changed to in use!");
                            tx.send(Event::NowInUse).unwrap()
                        }
                        (
                            State::SoakingOrAvailable {
                                counters,
                                since: _since,
                            },
                            State::Available,
                        ) => {
                            tx.send(Event::NowAvailable {
                                finished_cycle_stats: counters,
                            })
                            .unwrap();
                            log::info!("Changed to available!");
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    log::error!("{}", e);
                }
            }
        });

        Self {
            state: state.clone(),
            rx,
        }
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Available, // 6 minutes with 0 power
    InUse(InUseCounters),
    SoakingOrAvailable {
        counters: InUseCounters,
        since: std::time::Instant,
    },
}

impl State {
    pub fn to_new_state(&self, power: u32) -> State {
        match self {
            State::Available => {
                if power != 0 {
                    let now = std::time::Instant::now();
                    State::InUse(InUseCounters {
                        cycle_start: now,
                        last_power_measurement: power,
                        last_measurement_time: now,
                        energy_consumed_joules: 0,
                    })
                } else {
                    State::Available
                }
            }
            State::InUse(counters) => {
                let new_counter = counters.update(power);
                if power == 0 {
                    State::SoakingOrAvailable {
                        counters: new_counter,
                        since: std::time::Instant::now(),
                    }
                } else {
                    // not 0
                    State::InUse(new_counter)
                }
            }
            State::SoakingOrAvailable { counters, since } => {
                let new_counter = counters.update(power);
                if power == 0 {
                    if since.elapsed().as_secs() > IDLE_TIME_CONSIDER_DONE {
                        State::Available
                    } else {
                        State::SoakingOrAvailable {
                            counters: new_counter,
                            since: since.clone(),
                        }
                    }
                } else {
                    State::InUse(new_counter)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InUseCounters {
    pub cycle_start: std::time::Instant,
    pub last_power_measurement: u32,
    pub last_measurement_time: std::time::Instant,
    pub energy_consumed_joules: u32,
}

impl InUseCounters {
    pub fn update(&self, current_power: u32) -> Self {
        let avg_power = (self.last_power_measurement + current_power) / 2;
        let avg_power = avg_power as f64;
        let elapsed_time = self.last_measurement_time.elapsed().as_secs_f64();
        let joules = avg_power * elapsed_time;
        let total_joules = joules as u32 + self.energy_consumed_joules;
        Self {
            cycle_start: self.cycle_start,
            last_power_measurement: current_power,
            last_measurement_time: std::time::Instant::now(),
            energy_consumed_joules: total_joules,
        }
    }
}
