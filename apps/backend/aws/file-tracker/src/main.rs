use aws_config::{retry::RetryConfig, timeout::TimeoutConfig, SdkConfig};
use aws_lambda_events::sqs::SqsEvent;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_s3::Client as S3Client;
use flow_like_api::sea_orm::{ConnectOptions, Database};
use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
mod event_handler;
use std::time::Duration;

fn create_dynamo_client(config: &SdkConfig) -> DynamoClient {
    let retry_config = RetryConfig::standard()
        .with_max_attempts(5)
        .with_initial_backoff(Duration::from_millis(100));

    let timeout_config = TimeoutConfig::builder()
        .operation_timeout(Duration::from_secs(30))
        .build();

    let dynamo_config = aws_sdk_dynamodb::config::Builder::from(config)
        .retry_config(retry_config)
        .timeout_config(timeout_config)
        .build();

    DynamoClient::from_conf(dynamo_config)
}

fn create_s3_client(config: &SdkConfig) -> S3Client {
    let retry_config = RetryConfig::standard()
        .with_max_attempts(5)
        .with_initial_backoff(Duration::from_millis(100));

    let timeout_config = TimeoutConfig::builder()
        .operation_timeout(Duration::from_secs(30))
        .build();

    let s3_config = aws_sdk_s3::config::Builder::from(config)
        .retry_config(retry_config)
        .timeout_config(timeout_config)
        .build();

    S3Client::from_conf(s3_config)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut opt = ConnectOptions::new(db_url.to_owned());

    opt.max_connections(100)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8));

    let db = Database::connect(opt)
        .await
        .expect("Failed to connect to database");

    let config = aws_config::load_from_env().await;
    let dynamo = create_dynamo_client(&config);
    let s3 = create_s3_client(&config);

    run(service_fn(|event: LambdaEvent<SqsEvent>| {
        event_handler::function_handler(event, dynamo.clone(), s3.clone(), db.clone())
    }))
    .await
}
