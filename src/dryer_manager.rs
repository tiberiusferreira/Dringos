use crate::dryer_machine::OffState;
use crate::MsgType;
use std::fmt::{format, Debug};
use std::io::Error;

pub enum State {
    On(OnState),
    OffState(super::dryer_machine::OffState),
}

impl State {
    pub fn new() -> Self {
        Self::OffState(OffState::new())
    }
}

struct User {
    telegram_id: u64,
    db_id: i32,
    name: String,
    balance_reais: f64,
}

struct OnState {
    drier: super::dryer_machine::OnState,
    start_time: std::time::Instant,
    user: User,
}

pub struct DryerManager {
    state: Option<State>,
}

impl DryerManager {
    pub fn new() -> Self {
        Self {
            state: Some(State::new()),
        }
    }
    pub fn get_status_message(&self) -> String {
        return match &self.state {
            None => "A secadora está livre!".to_string(),
            Some(state) => match state {
                State::On(on) => {
                    format!(
                        "{} está usando a secadora há: {}",
                        on.user.name,
                        crate::seconds_to_hour_format(on.start_time.elapsed().as_secs())
                    )
                }
                State::OffState(_) => "A secadora está livre!".to_string(),
            },
        };
    }
    pub fn handle_turn_on_message(
        &mut self,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> Result<String, String> {
        if db_user.account_balance <= 2. {
            return Ok("Você precisa de ao menos 2 reais para ligar a secadora".to_string());
        }
        let map_err = |err: std::io::Error| {
            log::error!("{:#?}", err);
            "Erro de hardware :(. Fale com @TiberioFerreira"
        };
        let state = match self.state.take() {
            None => State::new(),
            Some(state) => state,
        };
        match state {
            State::On(on) => {
                if on.user.telegram_id == user.user_id {
                    Ok(format!("Você já está usando a secadora.",))
                } else {
                    Ok(format!(
                        "{} está usando a secadora há: {}",
                        on.user.name,
                        crate::seconds_to_hour_format(on.start_time.elapsed().as_secs())
                    ))
                }
            }
            State::OffState(drier_state) => {
                let drier = drier_state.turn_on().map_err(map_err)?;
                self.state = Some(State::On(OnState {
                    drier,
                    start_time: std::time::Instant::now(),
                    user: User {
                        telegram_id: user.user_id,
                        db_id: db_user.id,
                        name: db_user.name,
                        balance_reais: db_user.dryer_balance_reais,
                    },
                }));
                Ok(format!(
                    "Ligada, você tem R${:.2}",
                    db_user.dryer_balance_reais
                ))
            }
        }
    }
    pub fn handle_telegram_msg(
        &mut self,
        user_msg: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> String {
        match user_msg.update {
            MsgType::GenericMsg => self.get_status_message(),
            MsgType::TurnOn => {
                return match self.handle_turn_on_message(user_msg, db_user) {
                    Ok(s) => s,
                    Err(s) => s,
                }
            }
            MsgType::Update => self.get_status_message(),
        }
    }
}
