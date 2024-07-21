use config::{self, Config, File, FileFormat};
use duxcore::error::Error;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DuxConfigScalableController {
    pub rabbitmq: RabbitMqConfig,
    pub encryption: EncryptionParameters
}

impl DuxConfigScalableController {
    pub fn default() -> DuxConfigScalableController {
        DuxConfigScalableController {
            rabbitmq: RabbitMqConfig::default(),
            encryption: EncryptionParameters::default()
        }
    }

    pub fn from(path: Option<String>) -> Result<DuxConfigScalableController, Error> {
        let config_file_path = match path {
            Some(content) => content,
            None => "/etc/dux/dux.conf".to_string(),
        };

        let config_builder = Config::builder()
            .add_source(File::new(config_file_path.as_str(), FileFormat::Ini))
            .build();

        match config_builder {
            Ok(config_content) => {
                let dux_config = config_content.try_deserialize::<DuxConfigScalableController>();

                match dux_config {
                    Ok(config) => Ok(config),
                    Err(e) => {
                        // TODO : in this case, the user provided a config file but the parsing failed -> use default values instead of stopping everythin ?
                        Err(Error::FailureToParseContent(format!("{e}")))
                    }
                }
            }
            Err(_e) => {
                // TODO : Log some warning with 'e' content
                Ok(DuxConfigScalableController::default())
            }
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct RabbitMqConfig {
    pub rmq_address: String,
    pub rmq_port: u16,
    pub rmq_username: String,
    pub rmq_password: String,
}

impl RabbitMqConfig {
    pub fn default() -> RabbitMqConfig {
        RabbitMqConfig {
            rmq_address: "localhost".to_string(),
            rmq_port: 5672,
            rmq_username: "guest".to_string(),
            rmq_password: "guest".to_string(),
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct EncryptionParameters {
    pub password: String,
}

impl EncryptionParameters {
    pub fn default() -> EncryptionParameters {
        EncryptionParameters {
            password: "dux".to_string(),
        }
    }
}
