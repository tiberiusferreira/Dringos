use sqlx::PgPool;
use std::time::Duration;

pub struct Database {
    con: PgPool,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub telegram_id: i64,
    pub account_balance: f64,
    pub is_resident: bool,
    pub dryer_balance_reais: f64,
}

impl Database {
    pub async fn new() -> Self {
        loop {
            let con =
                sqlx::postgres::PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await;
            match con {
                Ok(con) => return Self { con },
                Err(e) => {
                    log::error!("{:#?}", e);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }
    pub async fn get_db_id(&self, telegram_id: u64) -> Result<Option<User>, sqlx::Error> {
        let telegram_id =
            i64::try_from(telegram_id).expect("Error converting telegram id from u64 to i64");
        sqlx::query_as!(
            User,
            "select * from coffeezera_users where telegram_id=$1",
            telegram_id
        )
        .fetch_optional(&self.con)
        .await
    }
}
