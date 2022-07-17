#[derive(serde::Deserialize, Debug, Clone)]
pub struct Settings {
    pub application_port: u16,
    pub jaeger_agent_endpoint: String
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("config/configuration"))?;
    settings.try_deserialize()
}