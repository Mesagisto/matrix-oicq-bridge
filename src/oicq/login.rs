use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rand::SeedableRng;
use ricq::{LoginResponse, QRCodeState};
use tracing::info;

pub async fn login(
  oicq_id: i64,
  password: Option<ArcStr>,
  _device: Option<String>,
) -> anyhow::Result<Arc<ricq::Client>> {
  let device_path = format!("data/devices/{}.json", oicq_id);
  let device_path = Path::new(device_path.as_str());
  let device = match device_path.exists() {
    true => serde_json::from_str(
      &tokio::fs::read_to_string(device_path)
        .await
        .expect("failed to read device.json"),
    )
    .expect("failed to parse device info"),
    false => {
      let mut seed = rand::prelude::StdRng::seed_from_u64(oicq_id as u64);
      let d = ricq::device::Device::random_with_rng(&mut seed);
      // let d = ricq::device::Device::random();
      tokio::fs::create_dir_all(device_path.parent().unwrap()).await?;
      tokio::fs::write(device_path, serde_json::to_string(&d).unwrap())
        .await
        .expect("failed to write device info to file");
      d
    }
  };
  let client = Arc::new(ricq::Client::new(
    device,
    ricq::version::get_version(ricq::version::Protocol::IPad),
    ricq::handler::DefaultHandler,
  ));
  let stream = tokio::net::TcpStream::connect(client.get_address())
    .await
    .expect("failed to connect");

  let clone_client = client.clone();
  let handle = tokio::spawn(async move { clone_client.start(stream).await });

  tokio::task::yield_now().await; // 等一下，确保连上了
  let token_path = format!("data/tokens/{}", oicq_id);
  let token_path = Path::new(token_path.as_str());
  if !token_path.parent().unwrap().exists() {
    tokio::fs::create_dir_all(token_path.parent().unwrap()).await?;
  }
  match token_path.exists() {
    true => {
      token(client.clone(), token_path).await?;
    }
    false => match password {
      Some(_) => passwd(client.clone(), oicq_id, password.unwrap()).await,
      None => qrcode(oicq_id, client.clone()).await?,
    },
  }

  ricq::ext::common::after_login(&client).await;
  remember_token(client.clone(), token_path).await?;
  {
    // tracing::info!("{:?}", client.get_friend_list().await);
    // tracing::info!("{:?}", client.get_group_list().await);
  }
  let d = client.get_allowed_clients().await;
  tracing::info!("{:?}", d);

  handle.await.unwrap();
  Ok(client)
}

pub async fn qrcode(oicq_id: i64, client: Arc<ricq::Client>) -> anyhow::Result<()> {
  info!("login with qrcode");

  let resp = client.fetch_qrcode().await.expect("failed to fetch qrcode");

  let qrcode_path = format!("data/qrcodes/{}.png", oicq_id);
  let qrcode_path = Path::new(qrcode_path.as_str());

  use ricq::ext::login::auto_query_qrcode;
  match resp {
    QRCodeState::ImageFetch(ricq::QRCodeImageFetch {
      ref image_data,
      ref sig,
    }) => {
      if !qrcode_path.parent().unwrap().exists() {
        std::fs::create_dir_all(qrcode_path.parent().unwrap())?;
      } else if qrcode_path.exists() {
        std::fs::remove_file(qrcode_path).ok();
      }
      tokio::fs::write(qrcode_path, &image_data)
        .await
        .expect("failed to write file");
      if let Err(err) = auto_query_qrcode(&client, sig).await {
        panic!("登录失败 {}", err)
      };
    }
    _ => {
      panic!("resp error")
    }
  }
  Ok(())
}

pub async fn token(client: Arc<ricq::Client>, path: &Path) -> anyhow::Result<()> {
  info!("login with token");

  let token_bytes = tokio::fs::read(path).await?;
  let token: ricq::client::Token = serde_cbor::from_slice(&token_bytes)?;
  let resp = client
    .token_login(token)
    .await
    .expect("failed to login with token");

  tracing::info!("{:?}", resp);
  Ok(())
}

pub async fn remember_token(client: Arc<ricq::Client>, path: &Path) -> anyhow::Result<()> {
  let token = client.gen_token().await;
  let token_bytes = serde_cbor::to_vec(&token)?;
  tokio::fs::write(path, token_bytes).await?;
  tracing::info!("token saved to {}", path.display());
  Ok(())
}

pub async fn passwd(client: Arc<ricq::Client>, oicq_id: i64, pass: ArcStr) {
  info!("login with passwd");

  let mut resp = client
    .password_login(oicq_id, &pass)
    .await
    .expect("failed to login with password");
  loop {
    match resp {
      LoginResponse::Success(ricq::LoginSuccess {
        ref account_info, ..
      }) => {
        tracing::info!("login success: {:?}", account_info);
        break;
      }
      LoginResponse::DeviceLocked(ricq::LoginDeviceLocked {
        ref sms_phone,
        ref verify_url,
        ref message,
        ..
      }) => {
        tracing::info!("device locked: {:?}", message);
        tracing::info!("sms_phone: {:?}", sms_phone);
        tracing::info!("verify_url: {:?}", verify_url);
        tracing::info!("手机打开url, 处理完成后重启程序");
        std::process::exit(0);
        // 也可以走短信验证
        // resp = client.request_sms().await.expect("failed to request sms");
      }
      LoginResponse::NeedCaptcha(ricq::LoginNeedCaptcha {
        ref verify_url,
        // 图片应该没了
        image_captcha: ref _image_captcha,
        ..
      }) => {
        tracing::info!("滑块URL: {:?}", verify_url);
        tracing::info!("请输入ticket:");
        let mut reader = tokio_util::codec::FramedRead::new(
          tokio::io::stdin(),
          tokio_util::codec::LinesCodec::new(),
        );
        let ticket = futures::StreamExt::next(&mut reader)
          .await
          .transpose()
          .expect("failed to read ticket")
          .expect("failed to read ticket");
        resp = client
          .submit_ticket(&ticket)
          .await
          .expect("failed to submit ticket");
      }
      LoginResponse::DeviceLockLogin { .. } => {
        resp = client
          .device_lock_login()
          .await
          .expect("failed to login with device lock");
      }
      LoginResponse::AccountFrozen => {
        panic!("account frozen");
      }
      LoginResponse::TooManySMSRequest => {
        panic!("too many sms request");
      }
      LoginResponse::UnknownStatus(ricq::LoginUnknownStatus {
        ref status,
        ref tlv_map,
        ref message,
      }) => {
        panic!(
          "unknown login status: {:?}, {:?}, {:?}",
          message, status, tlv_map
        );
      }
    }
  }
}
