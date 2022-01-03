use crate::washing_machine::{Event, State, WashingMachine};
use flexi_logger::{Age, Logger};
use frankenstein::{Api, ChatId, SendMessageParams, TelegramApi};
use once_cell::sync::Lazy;
use std::time::Duration;

mod washing_machine;

const CHANNEL_ID: once_cell::sync::Lazy<i64> = Lazy::new(|| {
    std::env::var("CHANNEL_ID")
        .expect("No CHANNEL_ID")
        .parse()
        .expect("CHANNEL_ID was not an integer")
});

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

fn main() {
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

    let token = std::env::var("API_TOKEN").expect("No API TOKEN");
    assert_ne!(*CHANNEL_ID, 0); // eager load it, so we don't get surprises later
    let api = frankenstein::Api::new(&token);
    let washer = washing_machine::WashingMachine::new();
    std::thread::sleep(Duration::from_secs(1)); // washing machine power meter startup
    let mut id_last_update_handled = 0;
    send_msg(&api, "Starting up!".to_string(), *CHANNEL_ID);
    loop {
        if let Err(e) = clear_pending_updates(&api) {
            log::error!("Error clearing updates: {:#?}", e);
            std::thread::sleep(Duration::from_secs(5));
        } else {
            break;
        }
    }
    loop {
        id_last_update_handled =
            respond_pending_telegram_msgs(&api, id_last_update_handled, &washer).unwrap();
        if let Some(event) = washer.check_for_events() {
            match event {
                Event::NowAvailable {
                    finished_cycle_stats,
                } => {
                    let cycle_duration_s = finished_cycle_stats.cycle_start.elapsed().as_secs();
                    let energy = finished_cycle_stats.energy_consumed_joules;
                    let energy_kwh = energy as f64 / (3.6 * 10e6);
                    send_msg(
                        &api,
                        format!(
                            "Ciclo terminado depois de {}. {} j consumidos, equivalente a {:.4} kwh",
                            seconds_to_hour_format(cycle_duration_s), energy, energy_kwh
                        ),
                        *CHANNEL_ID,
                    );
                }
                Event::NowInUse => {
                    send_msg(&api, "Máquina em uso".to_string(), *CHANNEL_ID);
                }
            }
        }
    }
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
fn handle_single_update(
    api: &Api,
    update: frankenstein::Update,
    washer: &WashingMachine,
) -> Result<(), frankenstein::Error> {
    if let Some(msg) = update.message {
        let chat_id = msg.chat.id;
        let state = washer.get_state();
        let outgoing_msg = match state {
            State::Available => "Disponível".to_string(),
            State::InUse(counters) => {
                format!(
                    "Em uso há {}. Potência atual: {} W",
                    seconds_to_hour_format(counters.cycle_start.elapsed().as_secs()),
                    counters.last_power_measurement
                )
            }
            State::SoakingOrAvailable { counters, since } => {
                format!(
                    "Em uso há {}. Potência atual: {} W. De molho ou terminada há {}",
                    seconds_to_hour_format(counters.cycle_start.elapsed().as_secs()),
                    counters.last_power_measurement,
                    seconds_to_hour_format(since.elapsed().as_secs()),
                )
            }
        };
        if let Some(_sender_user) = msg.from {
            println!("Chat id = {}", chat_id);
            api.send_message(&SendMessageParams {
                chat_id: ChatId::Integer(chat_id),
                text: outgoing_msg,
                parse_mode: None,
                entities: None,
                disable_web_page_preview: None,
                disable_notification: None,
                protect_content: None,
                reply_to_message_id: None,
                allow_sending_without_reply: None,
                reply_markup: None,
            })
            .unwrap();
        }
    }
    Ok(())
}

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

fn respond_pending_telegram_msgs(
    api: &Api,
    update_id_offset: u32,
    washer: &WashingMachine,
) -> Result<u32, frankenstein::Error> {
    let update_params = frankenstein::GetUpdatesParams {
        offset: Some(update_id_offset),
        limit: None,
        timeout: Some(10),
        allowed_updates: Some(vec!["message".to_string()]),
    };
    let result = api.get_updates(&update_params)?;
    if result.result.is_empty() {
        Ok(update_id_offset)
    } else {
        let latest_update_id = result
            .result
            .iter()
            .map(|u| u.update_id)
            .max()
            .unwrap_or(update_id_offset);
        for u in result.result {
            handle_single_update(api, u, washer)?;
        }
        Ok(latest_update_id + 1)
    }
}
