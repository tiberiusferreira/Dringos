use crate::dryer_manager::TickOutcome;
use crate::telegram::{MsgType, OutgoingMessage};
use chrono::Timelike;
use flexi_logger::{Age, Logger};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

mod database;
mod dryer_machine;
mod dryer_manager;
mod telegram;

fn seconds_to_hour_format(total_seconds: u64) -> String {
    let hours = total_seconds / (60 * 60);
    let remainer_hours = total_seconds % (60 * 60);
    let minutes = remainer_hours / 60;
    let remainer_minutes = remainer_hours % 60;
    let seconds = remainer_minutes % 60;
    format!("{}h{}m{}s", hours, minutes, seconds)
}

#[test]
fn seconds_to_hours_works() {
    assert_eq!("0h0m30s", seconds_to_hour_format(30));
    assert_eq!("0h1m30s", seconds_to_hour_format(90));
    assert_eq!("1h6m40s", seconds_to_hour_format(4000));
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    Logger::try_with_str("dringos=trace")
        .unwrap() // Write all error, warn, and info messages
        .format(flexi_logger::detailed_format)
        .log_to_file(flexi_logger::FileSpec::default().directory("./logs"))
        .rotate(
            // If the program runs long enough,
            flexi_logger::Criterion::AgeOrSize(Age::Day, 10_000_000), // - create a new file every day or every 10 MB
            flexi_logger::Naming::Timestamps, // - let the rotated files have a timestamp in their name
            flexi_logger::Cleanup::KeepLogFiles(7), // - keep at most 7 log files
        )
        .start()
        .unwrap();
    log_panics::init();
    let token = std::env::var("API_TOKEN").expect("No API TOKEN");
    let telegram_recv = telegram::Receiver::new(token.clone());
    let telegram_sender = telegram::Sender::new(token).start_sender_background_thread();
    let database = database::Database::new().await;
    let mut dryer = dryer_manager::DryerManager::new();
    let update_recv = telegram_recv.start_listening_for_updates_in_background_thread();

    loop {
        match update_recv.recv_timeout(Duration::from_secs_f32(0.5)) {
            Ok(user_message) => {
                let mut response = match database.get_db_id(user_message.user_id).await {
                    Ok(maybe_user) => match maybe_user {
                        None => {
                            OutgoingMessage {
                                update_message_with_id: None,
                                chat_id: user_message.chat_id,
                                text: format!("Voc?? n??o est?? registrado, mande essa mensagem para o @TiberioFerreira com o seu id: {}", user_message.user_id),
                                send_buttons: true,
                            }
                        }
                        Some(user) => {
                            dryer.handle_telegram_msg(user_message.clone(), user)
                        },
                    },
                    Err(e) => {
                        log::error!("{:#?}", e);
                        OutgoingMessage {
                            update_message_with_id: None,
                            chat_id: user_message.chat_id,
                            text: "Problema na rede interna da casa, fale com @TiberioFerreira".to_string(),
                            send_buttons: true,
                        }
                    }
                };
                let now = chrono::Local::now();
                response.text = format!(
                    "{} - Dados atualizados em: {:0>2}:{:0>2}:{:0>2}",
                    response.text,
                    now.hour(),
                    now.minute(),
                    now.second()
                );
                let updating_msg_with_same_text = user_message.message_text
                    == Some(response.text.clone())
                    && response.update_message_with_id.is_some();
                // ignore responses that would update a message with the same text as the original one
                if !updating_msg_with_same_text {
                    if let Err(e) = telegram_sender.try_send(response) {
                        log::error!("{:#?}", e);
                    }
                }
            }
            Err(e) => {
                match e {
                    RecvTimeoutError::Timeout => {
                        // it's ok
                    }
                    RecvTimeoutError::Disconnected => {
                        panic!("Telegram thread panicked!")
                    }
                }
            }
        }
        let tick_outcome = dryer.tick();
        match tick_outcome {
            TickOutcome::TurnOffAndRemoveUserOutOfMoney(cycle_stats) => {
                let response = OutgoingMessage {
                    update_message_with_id: None,
                    chat_id: cycle_stats.user.chat_id,
                    text: format!(
                        "Ciclo terminado por falta de saldo depois de {cycle_time}. Custo: R${cost_reais:.2} referentes a {kwh:.2} kwh consumidos. Saldo remanescente de {balance:.2}.",
                        cycle_time=seconds_to_hour_format(cycle_stats.start_time.elapsed().as_secs()),
                        cost_reais=cycle_stats.total_consumed_and_discounted_reais,
                        kwh=cycle_stats.total_consumed_kwh(),
                        balance=cycle_stats.user.balance_reais,
                    ),
                    send_buttons: true,
                };
                if let Err(e) = telegram_sender.try_send(response) {
                    log::error!("{:#?}", e);
                }
            }
            TickOutcome::TurnedOffDueToIdleTooLong(cycle_stats) => {
                let response = OutgoingMessage {
                    update_message_with_id: None,
                    chat_id: cycle_stats.user.chat_id,
                    text: format!(
                        "Ciclo terminado depois de {cycle_time}. Custo: R${cost_reais:.2} referentes a {kwh:.2} kwh consumidos. Saldo remanescente de {balance:.2}.",
                        cycle_time=seconds_to_hour_format(cycle_stats.start_time.elapsed().as_secs()),
                        cost_reais=cycle_stats.total_consumed_and_discounted_reais,
                        kwh=cycle_stats.total_consumed_kwh(),
                        balance=cycle_stats.user.balance_reais,
                    ),
                    send_buttons: true,
                };
                if let Err(e) = telegram_sender.try_send(response) {
                    log::error!("{:#?}", e);
                }
            }
            TickOutcome::DiscountConsumed {
                delta_energy_kwh: _delta_energy_kwh,
                delta_consumed_reais,
                user,
            } => {
                match database
                    .update_user_balance(user.telegram_id, delta_consumed_reais)
                    .await
                {
                    Ok(new_balance) => {
                        dryer.set_user_balance_reais(new_balance);
                    }
                    Err(e) => {
                        dryer.emergency_turn_off();
                        panic!("DB error updating user balance!{:#?}", e);
                    }
                }
            }
            TickOutcome::Off | TickOutcome::NotEnoughConsumptionToDiscountYet => {}
        }
    }
}
