#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum CargoEnv {
    Development,
    Production,
}

#[derive(clap::Parser)]
#[allow(clippy::module_name_repetitions)]
pub struct AppConfig {
    #[clap(long, env, value_enum)]
    pub cargo_env: CargoEnv,

    #[clap(long, env, default_value = "127.0.0.1")]
    pub app_host: String,

    #[clap(long, env, default_value = "8080")]
    pub app_port: u16,

    #[clap(
        long,
        env,
        default_value = "postgresql://admin:admin@localhost:5432/rust?schema=public"
    )]
    pub postgres_uri: String,
}
