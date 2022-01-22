use frankenstein::{
    Api, CallbackQuery, ChatId, Error, InlineKeyboardButton, InlineKeyboardMarkup, Message,
    TelegramApi, User,
};
use std::sync::mpsc::RecvError;
use std::time::Duration;

const TURN_ON: &'static str = "Turn On";
const UPDATE: &'static str = "Update";

/// WARNING, there can be only one receiver at any given time
pub struct Receiver {
    api: Api,
    id_last_update_handled: u32,
}

#[derive(Debug, Clone)]
pub struct UserMessage {
    pub user_id: u64,
    pub chat_id: i64,
    pub update: MsgType,
}

#[derive(Debug, Clone)]
pub enum MsgType {
    GenericMsg,
    TurnOn,
    Update,
}

pub struct Sender {
    api: Api,
}

#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    chat_id: i64,
    text: String,
    send_buttons: bool,
}

impl Sender {
    pub fn new(token: String) -> Self {
        let api = frankenstein::Api::new(&token);
        Self { api }
    }
    pub fn start_sender_background_thread(self) -> std::sync::mpsc::SyncSender<OutgoingMessage> {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<OutgoingMessage>(50);
        std::thread::Builder::new()
            .name("TelegramMessageSender".to_string())
            .spawn(move || loop {
                let msg = receiver.recv().expect("Telegram Sender disconnected");
                if let Err(e) = self.send_generic_msg(msg) {
                    log::error!("{:#?}", e);
                }
            })
            .expect("Couldn't start Telegram Sender Thread");
        sender
    }
    pub fn send_generic_msg(
        &self,
        outgoing_msg: OutgoingMessage,
    ) -> Result<(), frankenstein::Error> {
        let OutgoingMessage {
            chat_id,
            text,
            send_buttons,
        } = outgoing_msg;
        let reply_markup = if send_buttons {
            Some(frankenstein::ReplyMarkup::InlineKeyboardMarkup(
                InlineKeyboardMarkup {
                    inline_keyboard: vec![vec![
                        InlineKeyboardButton {
                            text: "Ligar".to_string(),
                            url: None,
                            login_url: None,
                            callback_data: Some(TURN_ON.to_string()),
                            switch_inline_query: None,
                            switch_inline_query_current_chat: None,
                            callback_game: None,
                            pay: None,
                        },
                        InlineKeyboardButton {
                            text: "Atualizar".to_string(),
                            url: None,
                            login_url: None,
                            callback_data: Some(UPDATE.to_string()),
                            switch_inline_query: None,
                            switch_inline_query_current_chat: None,
                            callback_game: None,
                            pay: None,
                        },
                    ]],
                },
            ))
        } else {
            None
        };

        let msg = frankenstein::SendMessageParams {
            chat_id: ChatId::Integer(chat_id),
            text,
            parse_mode: None,
            entities: None,
            disable_web_page_preview: None,
            disable_notification: None,
            protect_content: None,
            reply_to_message_id: None,
            allow_sending_without_reply: None,
            reply_markup,
        };
        self.api.send_message(&msg)?;
        Ok(())
    }
}

impl Receiver {
    pub fn new(token: String) -> Receiver {
        let api = frankenstein::Api::new(&token);
        Self {
            api,
            id_last_update_handled: 0,
        }
    }

    pub fn start_listening_update_in_background_thread(mut self) {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<UserMessage>(50);
        std::thread::Builder::new()
            .name("TelegramUpdateReceiver".to_string())
            .spawn(move || loop {
                match self.get_updates() {
                    Ok(updates) => {
                        for u in updates {
                            sender
                                .send(u)
                                .expect("Receiver is disconnected, main thread probably panicked!");
                        }
                    }
                    Err(e) => {
                        let sleep_s = 10;
                        log::error!("Error, sleeping {}s.\n{:#?}", sleep_s, e);
                        std::thread::sleep(Duration::from_secs(sleep_s));
                    }
                }
            })
            .expect("Couldn't start Telegram Receiver Thread");
    }

    fn get_updates(&mut self) -> Result<Vec<UserMessage>, frankenstein::Error> {
        let mut update_params = frankenstein::GetUpdatesParams {
            offset: Some(self.id_last_update_handled),
            limit: None,
            timeout: Some(10),
            allowed_updates: Some(vec!["message".to_string(), "callback_query".to_string()]), // None == all
        };
        let updates = self.api.get_updates(&update_params)?;
        if updates.result.is_empty() {
            return Ok(vec![]);
        }
        let last_update_id = updates
            .result
            .iter()
            .map(|u| u.update_id)
            .max()
            .unwrap_or(0);
        self.id_last_update_handled = last_update_id + 1;
        let parsed_updates: Vec<UserMessage> = updates
            .result
            .into_iter()
            .filter_map(|u| {
                if let Some(msg) = &u.message {
                    return match &msg.from {
                        None => {
                            log::warn!("Message Update with no From! {:#?}", u);
                            None
                        }
                        Some(from) => Some(UserMessage {
                            user_id: from.id,
                            chat_id: msg.chat.id,
                            update: MsgType::GenericMsg,
                        }),
                    };
                };
                match u.callback_query {
                    None => return None,
                    Some(callback) => {
                        let msg = callback.message?;
                        let data = callback.data?;
                        let update = match data.as_str() {
                            TURN_ON => MsgType::TurnOn,
                            UPDATE => MsgType::Update,
                            _ => return None,
                        };
                        Some(UserMessage {
                            user_id: callback.from.id,
                            chat_id: msg.chat.id,
                            update,
                        })
                    }
                }
            })
            .collect();
        Ok(parsed_updates)
    }
}
