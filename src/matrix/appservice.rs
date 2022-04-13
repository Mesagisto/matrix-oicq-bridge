use anyhow::Ok;
use arcstr::ArcStr;
use lateinit::LateInit;
use matrix_sdk_appservice::{
  matrix_sdk::{
    config::SyncSettings,
    event_handler::Ctx,
    room::Room,
    ruma::{
      api::{
        appservice::Namespace,
        appservice::{Namespaces, Registration, RegistrationInit},
      },
      events,
      events::room::member::{MembershipState, OriginalSyncRoomMemberEvent},
      UserId,
    },
    Client, LoopCtrl,
  },
  ruma::{
    api::client::message::send_message_event::v3::Request as SendMessageRequest,
    events::room::message::RoomMessageEventContent, TransactionId,
  },
  ruma::{events::room::message::SyncRoomMessageEvent, MilliSecondsSinceUnixEpoch, UInt},
  AppService, AppServiceRegistration,
};

use crate::CONFIG;
use tracing::trace;

static BOT_NAME: LateInit<ArcStr> = LateInit::new();

pub static MATRIX_BOT: LateInit<Client> = LateInit::new();
pub static MATRIX_APPSERVICE: LateInit<AppService> = LateInit::new();

pub async fn handle_room_member(
  appservice: AppService,
  room: Room,
  event: OriginalSyncRoomMemberEvent,
) -> anyhow::Result<()> {
  if !appservice.user_id_is_in_namespace(&event.state_key) {
    trace!("not an appservice user: {}", event.state_key);
  } else if let MembershipState::Invite = event.content.membership {
    let user_id = UserId::parse(event.state_key.as_str())?;
    let client = appservice.virtual_user_client(user_id.localpart()).await?;
    client.join_room_by_id(room.room_id()).await?;
  }

  Ok(())
}

pub async fn run() -> anyhow::Result<()> {
  let prefix = CONFIG.matrix.prefix.clone();
  let server_name = CONFIG.matrix.server_name.clone();
  let bot_name = arcstr::format!("{}_bot", prefix);
  BOT_NAME.init(bot_name.clone());
  let namespaces = {
    let mut value = Namespaces::new();
    value.users = vec![Namespace::new(
      true,
      format!(r"@{}_.*:{}", prefix, server_name),
    )];
    value.aliases = vec![Namespace::new(
      true,
      format!(r"#{}_.*:{}", prefix, server_name),
    )];
    value
  };
  let homeserver_url = CONFIG.matrix.homeserver_url.clone();
  let server_name = CONFIG.matrix.server_name.clone();
  let reg: Registration = RegistrationInit {
    id: CONFIG.matrix.id.to_string(),
    url: homeserver_url.to_string(),
    // TODO random key pair generation, maybe use uuid?
    as_token: "shoudleberandom2132471".to_string(),
    hs_token: "shoudleberandom4389023".to_string(),
    sender_localpart: BOT_NAME.to_string(),
    namespaces,
    rate_limited: Some(false),
    protocols: None,
  }
  .into();
  let reg_str = serde_yaml::to_string(&reg)?;
  std::fs::write("config/registration.yaml", reg_str)?;
  let registration = AppServiceRegistration::from(reg);
  let appservice =
    AppService::new(homeserver_url.as_str(), server_name.as_str(), registration).await?;
  appservice
    .register_user_query(Box::new(|_, _| Box::pin(async { true })))
    .await;
  appservice
    .register_event_handler_context(appservice.clone())?
    .register_event_handler(
      move |event: OriginalSyncRoomMemberEvent, room: Room, Ctx(appservice): Ctx<AppService>| {
        handle_room_member(appservice, room, event)
      },
    )
    .await?;
  MATRIX_APPSERVICE.init(appservice);
  let bot = MATRIX_APPSERVICE.virtual_user_client(bot_name).await?;
  MATRIX_BOT.init(bot.clone());

  tokio::spawn(async move {
    MATRIX_APPSERVICE
      .run("0.0.0.0", CONFIG.matrix.port)
      .await
      .unwrap();
  });

  tokio::spawn(async move {
    MATRIX_BOT
      .sync_with_callback(SyncSettings::default(), |resp| async {
        for (id, _) in resp.rooms.invite {
          let room = MATRIX_BOT.get_invited_room(&id).unwrap();
          room.accept_invitation().await.unwrap();
        }
        LoopCtrl::Continue
      })
      .await;
  });
  MATRIX_BOT
    .register_event_handler(
      |event: SyncRoomMessageEvent, room: Room, client: Client| async move {
        if event.sender().localpart() == BOT_NAME.as_str() {
          // get bot itself message
          return Ok(());
        }
        if event.origin_server_ts().0
          < MilliSecondsSinceUnixEpoch::now().0 - UInt::new(2000).unwrap()
        {
          // got former message,ignore it
          return Ok(());
        }
        let origin = event.into_full_event(room.room_id().to_owned());
        let origin = match origin {
          events::MessageLikeEvent::Original(v) => Ok(v),
          events::MessageLikeEvent::Redacted(_) => return Ok(()),
        }?;
        match origin.content.msgtype {
          events::room::message::MessageType::File(_) => todo!(),
          events::room::message::MessageType::Image(_) => todo!(),
          events::room::message::MessageType::Text(text) => {
            let txn_id = TransactionId::new();
            let content = RoomMessageEventContent::text_plain(format!("echo {}", text.body));
            let request = SendMessageRequest::new(room.room_id(), &txn_id, &content)?;
            let resp = client.send(request, None).await?;
            dbg!(resp);
          }
          _ => todo!(),
        }
        Ok(())
      },
    )
    .await;
  Ok(())
}
