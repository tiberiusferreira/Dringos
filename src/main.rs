// use crate::dryer_machine::{DryerMachine, Event, State};
use flexi_logger::{Age, Logger};
use frankenstein::{Api, ChatId, SendMessageParams, TelegramApi};
use once_cell::sync::Lazy;
use std::time::Duration;

mod dryer_machine;

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
    let con = sqlx::postgres::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let token = std::env::var("API_TOKEN").expect("No API TOKEN");
    // eager load it, so we don't get surprises later
    let api = frankenstein::Api::new(&token);

    // let dryer = dryer_machine::DryerMachine::new();
    // std::thread::sleep(Duration::from_secs(1)); // washing machine power meter startup
    let mut id_last_update_handled = 0;

    // Start
    // check if already in use
    // loop

    // loop {
    //     if let Err(e) = clear_pending_updates(&api) {
    //         log::error!("Error clearing updates: {:#?}", e);
    //         std::thread::sleep(Duration::from_secs(5));
    //     } else {
    //         break;
    //     }
    // }

    // let usb_port = "/dev/ttyUSB0";
    // let port = serialport::new(usb_port, 9600)
    //     .timeout(Duration::from_millis(2000))
    //     .open()
    //     .unwrap_or_else(|e| panic!("Cannot open `{}`: {}.", usb_port, e));
    // let mut pzem = dringos::pzemv3::Pzem::new(port);
    // loop {
    //     std::thread::sleep(Duration::from_millis(1000));
    //     println!("{:#?}", pzem.read_data().unwrap());
    //     pzem.reset_consumed_energy().unwrap();
    // }
    //
    // loop {
    //     match respond_pending_telegram_msgs(&api, id_last_update_handled, &washer) {
    //         Ok(res) => {
    //             id_last_update_handled = res;
    //         }
    //         Err(e) => {
    //             log::error!("{:#?}", e);
    //         }
    //     }
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
// fn clear_pending_updates(api: &Api) -> Result<(), frankenstein::Error> {
//     let mut update_params = frankenstein::GetUpdatesParams {
//         offset: Some(u32::MAX),
//         limit: None,
//         timeout: Some(10),
//         allowed_updates: None, // None == all
//     };
//     let updates = api.get_updates(&update_params)?;
//     let last_update_id = updates.result.iter().map(|u| u.update_id).max();
//     if let Some(last_update_id) = last_update_id {
//         update_params.offset = Some(last_update_id + 1);
//         api.get_updates(&update_params)?;
//     }
//     Ok(())
// }
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
