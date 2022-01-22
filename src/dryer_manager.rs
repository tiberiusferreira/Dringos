use crate::dryer_machine::OffState;
use crate::MsgType;

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

pub struct OnState {
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
    fn get_status_message(&self) -> String {
        return match &self.state {
            None => "A secadora está livre!".to_string(),
            Some(state) => match state {
                State::On(on) => {
                    format!(
                        "{} está usando a secadora há: {}.",
                        on.user.name,
                        crate::seconds_to_hour_format(on.start_time.elapsed().as_secs())
                    )
                }
                State::OffState(_) => "A secadora está livre!".to_string(),
            },
        };
    }

    fn turn_on_for_user(
        off_state: OffState,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> Result<OnState, (OffState, std::io::Error)> {
        off_state.turn_on().map(|on_state| OnState {
            drier: on_state,
            start_time: std::time::Instant::now(),
            user: User {
                telegram_id: user.user_id,
                db_id: db_user.id,
                name: db_user.name,
                balance_reais: db_user.dryer_balance_reais,
            },
        })
    }

    pub fn handle_turn_state_change(
        current: State,
        user: super::telegram::UserMessage,
        db_user: super::database::User,
    ) -> (State, String) {
        return match current {
            State::On(on) => {
                if on.user.telegram_id == user.user_id {
                    (State::On(on), format!("Você já está usando a secadora."))
                } else {
                    let msg = format!(
                        "{} está usando a secadora há: {}",
                        on.user.name,
                        crate::seconds_to_hour_format(on.start_time.elapsed().as_secs())
                    );
                    (State::On(on), msg)
                }
            }
            State::OffState(off_state) => {
                if db_user.account_balance <= 2. {
                    return (State::OffState(off_state), format!("Você precisa de ao menos 2 reais de saldo para ligar a secadora, você possui: R${:.2}.", db_user.account_balance));
                }
                match Self::turn_on_for_user(off_state, user, db_user.clone()) {
                    Ok(on_state) => (
                        State::On(on_state),
                        format!("Ligada, você tem R${:.2}", db_user.dryer_balance_reais),
                    ),
                    Err((off_state, err)) => {
                        log::error!("{:#?}", err);
                        (
                            State::OffState(off_state),
                            format!("Erro de hardware ao ligar a secadora :(, fale com @TiberioFerreira"),
                        )
                    }
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
    ) -> String {
        match user_msg.update {
            MsgType::GenericMsg => self.get_status_message(),
            MsgType::TurnOn => self.handle_turn_on_message(user_msg, db_user),
            MsgType::Update => self.get_status_message(),
        }
    }
}
