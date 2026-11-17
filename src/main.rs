mod avatar_pipeline;

use avatar_pipeline::{AvatarInput, InfraiClient, PipelineError};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: avatar-pipeline <user_id> <filename> <image_path>");
        std::process::exit(2);
    }
    let key = match std::env::var("INFRAI_API_KEY") {
        Ok(value) if !value.is_empty() => value,
        _ => {
            eprintln!("INFRAI_API_KEY is required");
            std::process::exit(2);
        }
    };
    let client = InfraiClient::new(key);
    let input = AvatarInput {
        user_id: args[1].clone(),
        filename: args[2].clone(),
        image_path: args[3].clone(),
    };
    match avatar_pipeline::run(&client, &input) {
        Ok(result) => println!("avatar stored: {}", result),
        Err(PipelineError::Business(message)) => {
            eprintln!("avatar rejected: {message}");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("request failed: {error}");
            std::process::exit(1);
        }
    }
}
