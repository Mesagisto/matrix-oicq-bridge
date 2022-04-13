use lateinit::LateInit;
use sqlx::{Pool, Postgres};

pub static DATABASE_POOL: LateInit<Pool<Postgres>> = LateInit::new();
