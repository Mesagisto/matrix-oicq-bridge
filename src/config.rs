use arcstr::literal;
use arcstr::ArcStr;

#[config_derive]
#[derive(AutomaticConfig)]
#[location = "config/bridge.yml"]
pub struct Config {
  #[educe(Default = false)]
  pub enable: bool,
  pub matrix: MatrixConfig,
  pub oicq: OicqConfig,
  pub database: DatabaseConfig,
}
#[config_derive]
pub struct MatrixConfig {
  #[educe(Default = 5899)]
  pub port: u16,
  #[educe(Default = "oicq")]
  pub prefix: ArcStr,
  #[educe(Default = "oicq-bridge")]
  pub id: ArcStr,
  #[educe(Default = "localhost")]
  pub server_name: ArcStr,
  #[educe(Default = "http://localhost:8008")]
  pub homeserver_url: ArcStr,
}
#[config_derive]
pub struct OicqConfig {
  #[educe(Default = 123456)]
  pub oicq_id: i64,
  #[educe(Default(expression = "Some(literal!(\"set-this-as ~ to use qrcode login\"))"))]
  pub passwd: Option<ArcStr>,
}
#[config_derive]
pub struct DatabaseConfig {
  #[educe(Default = "Only support postgresql for now")]
  pub __comment_url__: ArcStr,
  #[educe(Default = "postgres://postgres:password@localhost/oicq-bridge")]
  pub url: ArcStr,
}
