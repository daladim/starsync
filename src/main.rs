use std::error::Error;
use std::sync::mpsc;

use clap::{Args, Parser, Subcommand};

use starsync::source::list_sources;
use starsync::device::list_devices;
use starsync::sync::SyncManager;
use starsync::sync::status;


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List currently available source
    ListSources,
    /// List currently available devices
    ListDevices(ListDevicesArgs),
    /// Initializes a device so that it will be able to sync against a given source.
    Init(InitArgs),
    /// De-initializes a device (by removing its config files)
    Deinit(DeinitArgs),
    /// Sync an already inited device
    Sync(SyncArgs),
}

#[derive(Args)]
struct ListDevicesArgs {
    /// Only list devices that have been inited already
    #[arg(long)]
    already_inited: bool,
}

#[derive(Args)]
struct InitArgs {
    device: String,
    source: String,
}

#[derive(Args)]
struct DeinitArgs {
    device: String,
}

#[derive(Args)]
struct SyncArgs {
    device: String,
}


fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "debug")
    );

    let cli = Cli::parse();

    let res = match &cli.command {
        Commands::ListSources => cli_list_sources(),
        Commands::ListDevices(args) => cli_list_devices(args.already_inited),
        Commands::Init(args) => cli_init_device(args),
        Commands::Deinit(args) => cli_deinit_device(args),
        Commands::Sync(args) => cli_sync_device(args),
    };

    if let Err(err) = res {
        panic!("{}", err);
    }
}

fn cli_list_sources() -> Result<(), Box<dyn Error>> {
    let sources = list_sources();
    println!("Currently available sources:");
    for source in &sources {
        println!("  * {}", source.name());
    }
    println!("({} sources)", sources.len());

    Ok(())
}

fn cli_list_devices(only_already_inited: bool) -> Result<(), Box<dyn Error>>  {
    let devices = list_devices(only_already_inited);
    println!("Currently available devices:");
    for dev in &devices {
        println!("  * {}", dev.name());
        // TODO: show if they are inited (and with which source)
    }
    println!("({} devices)", devices.len());

    Ok(())
}

fn cli_init_device(args: &InitArgs) -> Result<(), Box<dyn Error>> {
    let config_display_path = starsync::init_device(&args.device, &args.source)?;
    println!("Successfully inited {}.", args.device);
    println!("You probably want to review the config at {} before starting a sync!", config_display_path);
    Ok(())
}

fn cli_deinit_device(args: &DeinitArgs) -> Result<(), Box<dyn Error>> {
    match starsync::deinit_device(&args.device) {
        Ok(()) => {
            println!("Successfully deinited {}", args.device);
            Ok(())
        },
        Err(starsync::DeinitError::NotInited) => {
            println!("Device {} is not inited", args.device);
            Ok(())
        },
        Err(err) => Err(err.into()),
    }
}

fn cli_sync_device(args: &SyncArgs) -> Result<(), Box<dyn Error>> {
    let (status_tx, status_rx) = starsync::sync::status::channel();
    let (validator_tx, validator_rx) = mpsc::channel();
    let (acknowledged_validator_tx, acknowledged_validator_rx) = mpsc::channel();
    let device_name = args.device.to_string();
    log::info!("Syncing {}...", device_name);

    let sync_thread = std::thread::spawn(move || {
        let _prevent_computer_going_to_sleep = starsync::PleaseStayAwake::new();

        let sync_manager = SyncManager::with_device(&device_name).unwrap(
            //
            //
            //
            //
            // TODO: fix this unwrap (at the same time as fixing the error/warning/status/Result<()> thing)
            //       note: this could be a NotInited
            //
        );
        sync_manager.start_sync(
            status_tx,
            validator_tx,
            acknowledged_validator_rx,
        )
    });

    // Wait for the validator to be sent
    let mut validator = validator_rx.recv().expect("sending half not to disconnect");

    if let Some((previous_hostname, current_hostname)) = &validator.last_sync_computer_mismatch {
        println!("Last sync was done on computer \"{}\" instead of the current computer \"{}\"", previous_hostname, current_hostname);
        print!("Do you still want to proceed? [y/n] ");
        let mut user_input = String::new();
        let stdin = std::io::stdin();
        stdin.read_line(&mut user_input)?;
        if user_input == "y" {
            validator.last_sync_computer_mismatch = None;
        }
    }

    // Send the acknowledged validator back
    acknowledged_validator_tx.send(validator).expect("transmission to be possible");

    loop {
        match status_rx.recv() {
            Err(_) => break,
            Ok(status::Message::Progress(starsync::sync::status::Progress::Done)) => {
                log::info!("Sync done.");
                break;
            },
            Ok(status::Message::Progress(prog)) => log::info!("===={:?}=====", prog),
            Ok(status::Message::Info(info)) => log::info!("{}", info),
            Ok(status::Message::Warning(warn)) => log::warn!("{}", warn),
            Ok(msg) => log::debug!("{:x?}", msg),
        }
    }

    match sync_thread.join() {
        Err(err) => std::panic::resume_unwind(err),
        Ok(Err(err)) => println!("Sync failed: {}", err),
        Ok(Ok(0)) => println!("Sync successfully completed."),
        Ok(Ok(n_warns)) => println!("Sync completed with {} warnings", n_warns),
    };

    Ok(())
}
