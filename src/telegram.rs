use frankenstein::{
    Api, CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, Message, TelegramApi,
    User,
};
const TURN_ON: &'static str = "Turn On";
const UPDATE: &'static str = "Update";
pub struct Telegram {
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

impl Telegram {
    pub fn new(token: String) -> Telegram {
        let api = frankenstein::Api::new(&token);
        Self {
            api,
            id_last_update_handled: 0,
        }
    }

    pub fn get_updates(&mut self) -> Result<Vec<UserMessage>, frankenstein::Error> {
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
    pub fn send_generic_msg(&self, chat_id: i64, text: String) -> Result<(), frankenstein::Error> {
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
            reply_markup: Some(frankenstein::ReplyMarkup::InlineKeyboardMarkup(
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
            )),
        };
        self.api.send_message(&msg)?;
        Ok(())
    }
}
