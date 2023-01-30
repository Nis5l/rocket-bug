#[macro_use]
extern crate rocket;

use rocket::fairing::AdHoc;
use sqlx::mysql::MySqlPoolOptions;
use rocket::{get, routes};
use rocket::fs::{FileServer, relative};
use std::sync::Arc;
use tokio::sync::Mutex;
use shared::card::packstats::data::PackStats;

mod sql;
mod config;
mod cors;
mod crypto;
mod shared;

#[get("/")]
fn index() -> &'static str {
    println!("index");
    "WaifuCollector API"
}

#[launch]
async fn rocket() -> _ {
    println!("Initializing config...");
    let config_figment = config::get_figment().expect("Initializing config failed");

    let config: config::Config = config_figment.extract().expect("Initializing config failed");

    println!("Connecting to database...");
    let sql = sql::Sql(MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&config.db_connection)
        .await.expect("Creating DB pool failed"));

    //TODO: paths not relative to start path
    println!("Setting up database...");
    for file in config.db_init_files.iter() {
        println!("-{}", file);
        sql::setup_db(&sql, file).await.expect("Failed setting up database");
    }

    println!("Initializing PackStats...");
    //cloning should be fine because it is implemented as Arc
    let mut pack_stats = PackStats::new(sql.clone(), &config).await.unwrap();
    pack_stats.init().await.expect("Error initializing PackStats");
    let pack_stats = Arc::new(Mutex::new(pack_stats));

    println!("Starting PackStats Thread...");
    {
        let pack_stats = pack_stats.clone();

        tokio::spawn(async {
            PackStats::start_thread(pack_stats).await
        });
    }

    rocket::custom(config_figment)
        .mount("/", routes![
           index,
        ])
        .mount(format!("/{}", &config.card_image_base), FileServer::from(relative!("static/card")))
        .mount(format!("/{}", &config.frame_image_base), FileServer::from(relative!("static/frame")))
        .mount(format!("/{}", &config.effect_image_base), FileServer::from(relative!("static/effect")))
        .mount(format!("/{}", &config.achievements_image_base), FileServer::from(relative!("static/achievements")))
        .mount(format!("/{}", &config.badges_image_base), FileServer::from(relative!("static/badges")))
        .register("/", vec![rocketjson::error::get_catcher()])
        .attach(AdHoc::config::<config::Config>())
        .attach(cors::CORS)
        .manage(sql)
        .manage(pack_stats)
}
