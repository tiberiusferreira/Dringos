use crate::dryer_machine::OffState;
use crate::{MsgType, OutgoingMessage};

const COST_REAIS_KWH: f64 = 1.1;
const TURN_OFF_SECONDS_ZERO_POWER_THRESHOLD: u64 = 60;
pub enum State {
    On(OnState),
    OffState(super::dryer_machine::OffState),
}

impl State {
    pub fn new() -> Self {
        Self::OffState(OffState::new())
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub telegram_id: u64,
    pub chat_id: i64,
    pub name: String,
    pub balance_reais: f64,
}

pub struct OnState {
    drier_on_state: super::dryer_machine::OnState,
    cycle_stats: CycleStats,
    start_time_zero_power: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub struct CycleStats {
    pub start_time: std::time::Instant,
    pub total_consumed_kwh: f64,
    pub total_consumed_reais: f64,
    pub user: User,
}

pub struct DryerManager {
    state: Option<State>,
}

pub enum TickOutcome {
    TurnOffAndRemoveUserOutOfMoney(CycleStats),
    TurnedOffDueToIdleTooLong(CycleStats),
    DiscountConsumed {
        delta_energy_kwh: f64,
        delta_consumed_reais: f64,
        user: User,
    },
    Off,
}

impl DryerManager {
    pub fn new() -> Self {
        Self {
            state: Some(State::new()),
        }
    }
    pub fn emergency_turn_off(&mut self) {
        let state = self.state.take().expect("State should have been initiated");
        let new_state = match state {
            State::On(on) => State::OffState(on.drier_on_state.turn_off_and_reset_energy_counter()),
            State::OffState(off) => State::OffState(off),
        };
        self.state = Some(new_state);
    }
    fn process_on_state_tick(mut on: OnState) -> (State, TickOutcome) {
        // check if user is out of money
        if on.cycle_stats.user.balance_reais <= 0.01 {
            let off = on.drier_on_state.turn_off_and_reset_energy_counter();
            return (
                State::OffState(off),
                TickOutcome::TurnOffAndRemoveUserOutOfMoney(on.cycle_stats),
            );
        }
        // if we are at "zero power" and we weren't before, mark it as being now
        let current_power = on.drier_on_state.get_current_power();
        if current_power <= 1. && on.start_time_zero_power.is_none() {
            on.start_time_zero_power = Some(std::time::Instant::now())
        }
        // if we are consuming power, make sure to remove the `start_time_zero_power`
        if current_power > 1. {
            on.start_time_zero_power = None;
        }

        if let Some(zero_power_since) = &on.start_time_zero_power {
            if TURN_OFF_SECONDS_ZERO_POWER_THRESHOLD <= zero_power_since.elapsed().as_secs() {
                // if we've been at "zero power" more time than the threshold, consider it
                // as turned off
                // turn off and remove user
                let off = on.drier_on_state.turn_off_and_reset_energy_counter();
                return (
                    State::OffState(off),
                    TickOutcome::TurnedOffDueToIdleTooLong(on.cycle_stats),
                );
            }
        }
        let consumed_wh = on.drier_on_state.get_consumed_energy_wh();
        let total_consumed_kwh = consumed_wh as f64 / 1000.;
        let total_consumed_reais = COST_REAIS_KWH * (total_consumed_kwh);
        let delta_energy_kwh = total_consumed_kwh - on.cycle_stats.total_consumed_kwh;
        let delta_consumed_reais = total_consumed_reais - on.cycle_stats.total_consumed_reais;
        assert!(
            delta_consumed_reais >= 0.,
            "Can't have negative consumed money"
        );
        assert!(
            delta_energy_kwh >= 0.,
            "Can't have negative consumed energy"
        );
        on.cycle_stats.total_consumed_kwh = total_consumed_kwh;
        on.cycle_stats.total_consumed_reais = total_consumed_reais;
        let user = on.cycle_stats.user.clone();
        (
            State::On(on),
            TickOutcome::DiscountConsumed {
                delta_energy_kwh,
                delta_consumed_reais,
                user,
            },
        )
    }
    pub fn set_user_balance_reais(&mut self, balance_reais: f64) {
        let current_state = self
            .state
            .as_mut()
            .expect("State should have been initiated");
        match current_state {
            State::On(on) => {
                on.cycle_stats.user.balance_reais = balance_reais;
            }
            State::OffState(_) => {
                panic!("Logic bug, can't set user when off!")
            }
        }
    }
    pub fn tick(&mut self) -> TickOutcome {
        let current_state = self.state.take().expect("State should have been initiated");
        let (state, tick_outcome) = match current_state {
            State::On(on_state) => Self::process_on_state_tick(on_state),
            State::OffState(off_state) => (State::OffState(off_state), TickOutcome::Off),
        };
        self.state = Some(state);
        tick_outcome
    }
    fn get_status_message(&self, telegram_id: u64) -> String {
        let state = self
            .state
            .as_ref()
            .expect("State should have been initiated");
        return match state {
            State::On(on) => {
                if on.cycle_stats.user.telegram_id == telegram_id {
                    let cycle_stats = &on.cycle_stats;
                    format!(
                        "Secando há {cycle_time}. Custo até o momento: R${cost_reais:.2} referentes a {kwh:.2} kwh consumidos. Saldo remanescente de {balance:.2}.",
                        cycle_time= crate::seconds_to_hour_format(cycle_stats.start_time.elapsed().as_secs()),
                        cost_reais=cycle_stats.total_consumed_reais,
                        kwh=cycle_stats.total_consumed_kwh,
                        balance=cycle_stats.user.balance_reais,
                    )
                } else {
                    format!(
                        "{} está usando a secadora há: {}.",
                        on.cycle_stats.user.name,
                        crate::seconds_to_hour_format(
                            on.cycle_stats.start_time.elapsed().as_secs()
                        )
                    )
                }
            }
            State::OffState(_) => "A secadora está livre!".to_string(),
        };
    }

    fn turn_on_for_user(
        off_state: OffState,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> OnState {
        let on = off_state.turn_on_and_reset_energy_counter();
        OnState {
            drier_on_state: on,
            start_time_zero_power: None,
            cycle_stats: CycleStats {
                start_time: std::time::Instant::now(),
                total_consumed_kwh: 0.0,
                total_consumed_reais: 0.0,
                user: User {
                    telegram_id: user.user_id,
                    chat_id: user.chat_id,
                    name: db_user.name,
                    balance_reais: db_user.balance_reais,
                },
            },
        }
    }

    pub fn handle_turn_state_change(
        current: State,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> (State, String) {
        return match current {
            State::On(on) => {
                if on.cycle_stats.user.telegram_id == user.user_id {
                    (State::On(on), format!("Você já está usando a secadora."))
                } else {
                    let msg = format!(
                        "{} está usando a secadora há: {}",
                        on.cycle_stats.user.name,
                        crate::seconds_to_hour_format(
                            on.cycle_stats.start_time.elapsed().as_secs()
                        )
                    );
                    (State::On(on), msg)
                }
            }
            State::OffState(off_state) => {
                if db_user.balance_reais <= 2. {
                    return (State::OffState(off_state), format!("Você precisa de ao menos 2 reais de saldo para ligar a secadora, você possui: R${:.2}.", db_user.balance_reais));
                } else {
                    let state = Self::turn_on_for_user(off_state, user, db_user.clone());
                    (
                        State::On(state),
                        format!("Ligada, você tem R${:.2}", db_user.balance_reais),
                    )
                }
            }
        };
    }

    fn handle_turn_on_message(
        &mut self,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> String {
        let state = self
            .state
            .take()
            .expect("State should have been initialized by now!");
        let (new_state, response) = Self::handle_turn_state_change(state, user, db_user);
        self.state = Some(new_state);
        response
    }

    pub fn handle_telegram_msg(
        &mut self,
        user_msg: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> OutgoingMessage {
        match user_msg.update {
            MsgType::GenericMsg => OutgoingMessage {
                update_message_with_id: None,
                chat_id: user_msg.chat_id,
                text: self.get_status_message(user_msg.user_id),
                send_buttons: true,
            },
            MsgType::TurnOn => OutgoingMessage {
                update_message_with_id: user_msg.message_id,
                chat_id: user_msg.chat_id,
                text: self.handle_turn_on_message(user_msg, db_user),
                send_buttons: true,
            },
            MsgType::Update => OutgoingMessage {
                update_message_with_id: user_msg.message_id,
                chat_id: user_msg.chat_id,
                text: self.get_status_message(user_msg.user_id),
                send_buttons: true,
            },
        }
    }
}
