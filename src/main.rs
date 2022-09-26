use data::DATABASE_POOL;
use sqlx::{postgres::PgPoolOptions, Database, Pool};
use tracing::{warn, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

use crate::config::CONFIG;

pub mod bridge;
pub mod config;
mod data;
pub mod db;
mod matrix;
mod oicq;

#[macro_use]
extern crate educe;
#[macro_use]
extern crate automatic_config;
#[macro_use]
extern crate singleton;
#[macro_use]
extern crate derive_builder;

#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
          // use local time
          time::UtcOffset::__from_hms_unchecked(8, 0, 0),
          time::macros::format_description!(
            "[year repr:last_two]-[month]-[day] [hour]:[minute]:[second]"
          ),
        )),
    )
    .with(
      tracing_subscriber::filter::Targets::new()
        .with_target("oicq_bridge", Level::DEBUG)
        .with_target("ricq", Level::DEBUG)
        .with_target("matrix_sdk", Level::DEBUG)
        .with_target("matrix_sdk_appservice", Level::DEBUG)
        .with_target("qrcode_login", Level::DEBUG)
        // .with_target("matrix_sdk::client", Level::ERROR)
        .with_default(Level::WARN),
    )
    .init();
  run().await.unwrap();
}
async fn run() -> anyhow::Result<()> {
  config::Config::reload().await?;
  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(CONFIG.database.url.as_str())
    .await?;
  DATABASE_POOL.init(pool);
  if !CONFIG.enable {
    warn!("Mesagisto-Bot is not enabled and is about to exit the program.");
    warn!("To enable it, please modify the configuration file.");
    warn!("Mesagisto-Bot未被启用, 即将退出程序。");
    warn!("若要启用，请修改配置文件。");
    return Ok(());
  }
  run_matrix().await.unwrap();
  run_oicq().await.unwrap();

  tokio::signal::ctrl_c().await.unwrap();
  CONFIG.save().await?;
  Ok(())
}
async fn run_matrix() -> anyhow::Result<()> {
  tokio::spawn(async move {
    matrix::appservice::run().await.unwrap();
  });
  Ok(())
}
async fn run_oicq() -> anyhow::Result<()> {
  let oicq_id = CONFIG.oicq.oicq_id;
  let passwd = CONFIG.oicq.passwd.clone();
  tokio::spawn(async move {
    oicq::login::login(oicq_id, passwd, None).await.unwrap();
  });
  Ok(())
}
