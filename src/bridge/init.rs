use std::{collections::VecDeque, process::Output, vec};

use arcstr::ArcStr;
use futures::{
  future::BoxFuture,
  stream::{self, ForEachConcurrent, FuturesUnordered},
  Future, FutureExt, Stream, StreamExt,
};
use ricq::client;

use crate::matrix::appservice::{MATRIX_APPSERVICE, MATRIX_BOT};

pub async fn register(client: &client::Client) -> anyhow::Result<()> {
  let users = client.get_friend_list().await?.friends;
  let mut users: Vec<OicqUserInfo> = users
    .into_iter()
    .map(|v| {
      return OicqUserInfoBuilder::default()
        // FIXME format from config
        .id(arcstr::format!("oicq_{}", v.uin))
        .nick(v.nick)
        .build()
        .unwrap();
    })
    .collect();
  let self_info = client.account_info.read().await;
  let self_info = OicqUserInfoBuilder::default()
    .id(arcstr::format!("oicq_{}", client.uin().await))
    .nick(self_info.nickname.clone())
    .build()?;
  users.push(self_info);
  register_users(users).await;
  Ok(())
}
#[derive(Default, Debug, Builder)]
#[builder(setter(into))]
pub struct OicqUserInfo {
  id: ArcStr,
  nick: ArcStr,
}

pub async fn register_rooms() {}
pub async fn register_users(users: Vec<OicqUserInfo>) -> Vec<anyhow::Result<()>> {
  users
    .into_iter()
    .map(register_user)
    .collect::<FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
}
pub async fn register_user(user: OicqUserInfo) -> anyhow::Result<()> {
  Ok(())
}
