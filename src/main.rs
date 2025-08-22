use std::time::Instant;

use tokio;
use log::{LevelFilter, info};
use log4rs;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};

mod args;


fn init_logging(args: &args::types::Args) -> Option<()> {
    match &args.log_config {
        Some(path) => {
            log4rs::init_file(path, Default::default()).ok()?;
        }
        None => {
            let stdout = ConsoleAppender::builder().build();

            let config = Config::builder()
                .appender(Appender::builder().build("stdout", Box::new(stdout)))
                .build(Root::builder().appender("stdout").build(LevelFilter::Debug)).ok()?;
            log4rs::init_config(config).ok()?;
        }
    }

    Some(())
}

fn print_hello() {
    println!(r" _____      _   _____             _            ");
    println!(r"|  __ \    | | |  __ \           | |           ");
    println!(r"| |__) |___| |_| |__) |___  _   _| |_ ___ _ __ ");
    println!(r"|  _  // __| __|  _  // _ \| | | | __/ _ \ '__|");
    println!(r"| | \ \\__ \ |_| | \ \ (_) | |_| | ||  __/ |   ");
    println!(r"|_|  \_\___/\__|_|  \_\___/ \__,_|\__\___|_|   ");
    println!(r"                                               ");
}


async fn init_and_run(args: &args::types::Args) {
    let start = Instant::now();

    if init_logging(&args).is_none() {
        println!("cannot initialize logging!");
        return;
    }

    print_hello();

    let duration = start.elapsed();
    info!("Server startup completed in {:?}", duration);
    info!("Starting server at http://{}:{}", args.bind, args.port);
}

fn main() {
    let args = args::types::get_args();
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r.block_on(async move { init_and_run(&args).await }),
        Err(e) => println!("{}", e),
    }
}
