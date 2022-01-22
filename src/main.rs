// use crate::dryer_machine::{DryerMachine, Event, State};
use crate::database::User;
use crate::dryer_manager::DryerManager;
use crate::telegram::{MsgType, UserMessage};
use flexi_logger::{Age, Logger};
use frankenstein::{Api, ChatId, Error, SendMessageParams, TelegramApi};
use once_cell::sync::Lazy;
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
    let mut telegram_recv = telegram::Receiver::new(token.clone());
    let mut telegram_sender = telegram::Sender::new(token);
    let db = database::Database::new().await;
    let mut dryer = dryer_manager::DryerManager::new();

    // loop {
    //     match telegram.get_updates() {
    //         Ok(updates) => {
    //             for u in updates {
    //                 let maybe_user = match db.get_db_id(u.user_id as i64).await {
    //                     Ok(maybe_user) => maybe_user,
    //                     Err(e) => {
    //                         log::error!("{:#?}", e);
    //                         continue;
    //                     }
    //                 };
    //                 let user = match maybe_user {
    //                     None => {
    //                         if let Err(e) = telegram.send_generic_msg(u.chat_id, format!("Você não está registrado. Envie essa mensagem para @TiberioFerreira com o seu id: {}", u.user_id)){
    //                             log::error!("{:#?}", e);
    //                         }
    //                         continue;
    //                     }
    //                     Some(user) => user,
    //                 };
    //                 if let Err(e) =
    //                     telegram.send_generic_msg(u.chat_id, dryer.handle_telegram_msg(u, user))
    //                 {
    //                     log::error!("{:#?}", e);
    //                 }
    //             }
    //         }
    //         Err(e) => {
    //             log::error!("{:#?}", e);
    //         }
    //     }
    //     println!("{:?}", telegram.get_updates().unwrap());
    // }
}

fn send_msg(api: &Api, msg: String, chat_id: i64) {
    if let Err(e) = api.send_message(&SendMessageParams {
        chat_id: ChatId::Integer(chat_id),
        text: msg,
        parse_mode: None,
        entities: None,
        disable_web_page_preview: None,
        disable_notification: None,
        protect_content: None,
        reply_to_message_id: None,
        allow_sending_without_reply: None,
        reply_markup: None,
    }) {
        log::error!("{:#?}", e);
    }
}

//
// fn handle_single_update(
//     api: &Api,
//     update: frankenstein::Update,
//     washer: &DryerMachine,
// ) -> Result<(), frankenstein::Error> {
//     if let Some(msg) = update.message {
//         let chat_id = msg.chat.id;
//         let state = washer.get_state();
//         let outgoing_msg = match state {
//             State::Available => "Disponível".to_string(),
//             State::InUse(counters) => {
//                 format!(
//                     "Em uso há {}. Potência atual: {} W",
//                     seconds_to_hour_format(counters.cycle_start.elapsed().as_secs()),
//                     counters.last_power_measurement
//                 )
//             }
//             State::SoakingOrAvailable { counters, since } => {
//                 format!(
//                     "Em uso há {}. Potência atual: {} W. De molho ou terminada há {}",
//                     seconds_to_hour_format(counters.cycle_start.elapsed().as_secs()),
//                     counters.last_power_measurement,
//                     seconds_to_hour_format(since.elapsed().as_secs()),
//                 )
//             }
//         };
//         if let Some(_sender_user) = msg.from {
//             println!("Chat id = {}", chat_id);
//             api.send_message(&SendMessageParams {
//                 chat_id: ChatId::Integer(chat_id),
//                 text: outgoing_msg,
//                 parse_mode: None,
//                 entities: None,
//                 disable_web_page_preview: None,
//                 disable_notification: None,
//                 protect_content: None,
//                 reply_to_message_id: None,
//                 allow_sending_without_reply: None,
//                 reply_markup: None,
//             })
//             .unwrap();
//         }
//     }
//     Ok(())
// }
//
fn clear_pending_updates(api: &Api) -> Result<(), frankenstein::Error> {
    let mut update_params = frankenstein::GetUpdatesParams {
        offset: Some(u32::MAX),
        limit: None,
        timeout: Some(10),
        allowed_updates: None, // None == all
    };
    let updates = api.get_updates(&update_params)?;
    let last_update_id = updates.result.iter().map(|u| u.update_id).max();
    if let Some(last_update_id) = last_update_id {
        update_params.offset = Some(last_update_id + 1);
        api.get_updates(&update_params)?;
    }
    Ok(())
}
//
// fn respond_pending_telegram_msgs(
//     api: &Api,
//     update_id_offset: u32,
//     washer: &DryerMachine,
// ) -> Result<u32, frankenstein::Error> {
//     let update_params = frankenstein::GetUpdatesParams {
//         offset: Some(update_id_offset),
//         limit: None,
//         timeout: Some(10),
//         allowed_updates: Some(vec!["message".to_string()]),
//     };
//     let result = api.get_updates(&update_params)?;
//     if result.result.is_empty() {
//         Ok(update_id_offset)
//     } else {
//         let latest_update_id = result
//             .result
//             .iter()
//             .map(|u| u.update_id)
//             .max()
//             .unwrap_or(update_id_offset);
//         for u in result.result {
//             handle_single_update(api, u, washer)?;
//         }
//         Ok(latest_update_id + 1)
//     }
// }
