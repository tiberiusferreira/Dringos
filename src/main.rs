use crate::telegram::{MsgType, OutgoingMessage};
use flexi_logger::{Age, Logger};
use std::sync::mpsc::TryRecvError;

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
    Logger::try_with_str("info")
        .unwrap() // Write all error, warn, and info messages
        .log_to_file(flexi_logger::FileSpec::default())
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
    let update_recv = telegram_recv.start_listening_update_in_background_thread();

    loop {
        match update_recv.try_recv() {
            Ok(user_message) => {
                let response = match database.get_db_id(user_message.user_id).await {
                    Ok(maybe_user) => match maybe_user {
                        None => {
                            OutgoingMessage {
                                chat_id: user_message.chat_id,
                                text: format!("Você não está registrado, mande essa mensagem para o @TiberioFerreira com o seu id: {}", user_message.user_id),
                                send_buttons: true,
                            }

                        }
                        Some(user) => {
                            let response = dryer.handle_telegram_msg(user_message.clone(), user);
                            OutgoingMessage{
                                chat_id: user_message.chat_id,
                                text: response,
                                send_buttons: true
                            }
                        },
                    },
                    Err(e) => {
                        log::error!("{:#?}", e);
                        OutgoingMessage {
                            chat_id: user_message.chat_id,
                            text: "Problema na rede interna da casa, fale com @TiberioFerreira".to_string(),
                            send_buttons: true,
                        }

                    }
                };
                if let Err(e) = telegram_sender.try_send(response) {
                    log::error!("{:#?}", e);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                panic!("Telegram thread panicked!")
            }
        }
    }
}
