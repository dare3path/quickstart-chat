#![allow(clippy::disallowed_macros)]
#![forbid(unsafe_code)]

mod module_bindings;
use module_bindings::*;

use spacetimedb_sdk::{credentials, DbContext, Error, Event, Identity, Status, Table, TableWithPrimaryKey};

use clap::{Arg, Command};
use std::path::PathBuf;


// ## Define the main function
use std::sync::{
    atomic::{AtomicBool, Ordering},
};

// This global AtomicBool is safe for concurrent access.
static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() {
    //log::info!("hi"); //not a thing
    // Create an atomic flag that can be safely shared among threads.
    //let running = Arc::new(AtomicBool::new(true));
    //let r = running.clone();

    // Set the Ctrl+C handler.
    ctrlc::set_handler(move || {
        // For Windows: "Ctrl+Z", and for non-Windows (Unix-like): "Ctrl+D"
        let end_key = if cfg!(windows) {
            "Ctrl+Z"
        } else {
            "Ctrl+D"
        };
        println!("Received Ctrl+C(or SIGINT, or SIGTERM, or SIGHUP)! Please use {} to exit gracefully, or press Enter to sense this and exit.", end_key);
        //r.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

     // Parse command-line arguments with clap
    let matches = Command::new("quickstart-chat")
        .arg(
            Arg::new("trust-server-cert")
                .long("trust-server-cert")
                .alias("cert")
                .value_name("FILE")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Path to PEM file containing certificates to trust for the server"),
        )
        .arg(
            Arg::new("client-cert")
                .long("client-cert")
                .value_name("FILE")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Path to the client’s certificate (PEM) for mTLS"),
        )
        .arg(
            Arg::new("client-key")
                .long("client-key")
                .value_name("FILE")
                .value_parser(clap::value_parser!(PathBuf))
                .requires("client-cert")
                .help("Path to the client’s private key (PEM) for mTLS"),
        )
        .arg(
            Arg::new("trust-system-certs")
                .long("trust-system-certs")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("no-trust-system-certs")
                .help("Use system root certificates (default)"),
        )
        .arg(
            Arg::new("no-trust-system-certs")
                .long("no-trust-system-certs")
                .alias("empty-trust-store")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("trust-system-certs")
                .help("Use empty trust store (requires --trust-server-cert)"),
        )
        .get_matches();
    //FIXME: check args./aliases

    //FIXME: see if this 'if' makes sense:
    // Validate no-trust-system-certs
    if matches.get_flag("no-trust-system-certs") && !matches.contains_id("trust-server-cert") {
        eprintln!("--no-trust-system-certs requires --trust-server-cert");
        std::process::exit(1);
    }

//    // ### Parse command-line arguments for --cert into a PathBuf
//    let args: Vec<String> = std::env::args().collect();
//    let cert_path: Option<PathBuf> = args.iter()
//        .position(|arg| arg == "--cert")
//        .map(|i| args.get(i + 1).expect("Missing certificate path after --cert"))
//        .map(|s| PathBuf::from(s));
//    // Connect to the database with optional cert
//    let ctx = connect_to_db(cert_path);

    // Extract arguments
    let trust_server_cert = matches.get_one::<PathBuf>("trust-server-cert").cloned();
    let client_cert = matches.get_one::<PathBuf>("client-cert").cloned();
    let client_key = matches.get_one::<PathBuf>("client-key").cloned();
    let trust_system_certs = if matches.get_flag("no-trust-system-certs") {
        Some(false)
    } else if matches.get_flag("trust-system-certs") {
        Some(true)
    } else {
        None // None here but deeper this means 'true', in db_connection.rs
    };

    // Connect to the database
    let ctx = connect_to_db(trust_server_cert, client_cert, client_key, trust_system_certs);

    // Register callbacks to run in response to database events.
    register_callbacks(&ctx);

    // Subscribe to SQL queries in order to construct a local partial replica of the database.
    subscribe_to_tables(&ctx);

    // Spawn a thread, where the connection will process messages and invoke callbacks.
    ctx.run_threaded();

    // Handle CLI input
    user_input_loop(&ctx);
    //gracefully exit, if Ctrl+D was pressed (Ctrl+Z on Windows)
    let _res=ctx.disconnect();
//    println!("disconnect result: {:?}", res);//always instantly Ok(()); unless it was called already
    //println!("Awaiting for disconnect to succeed, timeout 3 seconds...");
    const TIMEOUT:u64=3;
    let duration = std::time::Duration::from_secs(TIMEOUT);
    std::thread::sleep(duration);
    //not reached(unless problems?):
    println!("Failed to disconnect from the database! Waited {} seconds.", TIMEOUT);
}

// ## Connect to the database

/// The host and port, without scheme, of the SpacetimeDB instance hosting our chat module.
const HOST_PORT: &str = "127.1.2.3:6543";

/// The module name we chose when we published our module.
const DB_NAME: &str = "quickstart-chat";

/// Load credentials from a file and connect to the database.
fn connect_to_db(
    cert_path: Option<PathBuf>,
    client_cert: Option<PathBuf>,
    client_key: Option<PathBuf>,
    trust_system_certs: Option<impl Into<bool>>
    ) -> DbConnection {
    // ### Construct URI with scheme based on cert presence
    let expects_https=cert_path.is_some() || client_cert.is_some() || client_key.is_some();
    let scheme = if expects_https { "https" } else { "http" };
    let uri = format!("{}://{}", scheme, HOST_PORT);

    DbConnection::builder()
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(on_connected)
        // Register our `on_connect_error` callback, which will print a message, then exit the process.
        .on_connect_error(on_connect_error)
        // Our `on_disconnect` callback, which will print a message, then exit the process.
        .on_disconnect(on_disconnected)
        // If the user has previously connected, we'll have saved a token in the `on_connect` callback.
        // In that case, we'll load it and pass it to `with_token`,
        // so we can re-authenticate as the same `Identity`.
        .with_token(creds_store().load().expect("Error loading credentials"))
        // Set the database name we chose when we called `spacetime publish`.
        .with_module_name(DB_NAME)
        // Set the URI of the SpacetimeDB host that's running our database.
        .with_uri(&uri)
        // The local CA pub cert, or server's pub key, in PEM format, to can trust the server!
//        .with_trusted_cert(Some("../../../spacetimedb-cert-gen/ca.crt"))
        .with_trusted_cert(cert_path)
        // Add client identity (TLS)
        .with_client_cert(client_cert)
        .with_client_key(client_key)
        // Configure trust store
        .with_trust_system_certs(trust_system_certs)
        // Finalize configuration and connect!
        .build()
        //.expect("Failed to connect")
        .unwrap_or_else(|error| {
            use std::error::Error;
            let mut body_bytes: Option<Vec<u8>> = None;
            let mut charset: Option<String> = None;

            let mut current_error: &dyn Error = &error;
            while let Some(source) = current_error.source() {
                let debug_str = format!("{:#?}", source);
                if debug_str.contains("Http") && debug_str.contains("Response") {
                    // Extract charset from headers
                    if let Some(header_start) = debug_str.find("\"content-type\": \"") {
                        let header_section = &debug_str[header_start + 17..];
                        if let Some(header_end) = header_section.find('"') {
                            let content_type = &header_section[..header_end];
                            //eprintln!("!!!!content_type={:?}!!!!", content_type);
                            if let Some(charset_start) = content_type.find("charset=") {
                                let charset_value = &content_type[charset_start + 8..];
                                charset = Some(charset_value.to_lowercase());
                                //eprintln!("!!!!{:?}!!!!", charset);
                            }
                        }
                    }

                    // Extract body bytes
                    if let Some(body_start) = debug_str.find("body: Some(") {
                        let body_section = &debug_str[body_start + 11..];
                        if let Some(body_end) = body_section.find("]") {
                            let body_array = &body_section[..body_end];
                            let cleaned_array = body_array
                                .trim_start_matches(|c: char| c.is_whitespace() || c == '[')
                                .trim_end_matches(|c: char| c.is_whitespace() || c == ',');
                            let bytes: Vec<u8> = cleaned_array
                                .split(',')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .filter_map(|s| s.parse::<u8>().ok())
                                .collect();
                            body_bytes = Some(bytes);
                            break;
                        }
                    }
                }
                current_error = source;
            }

            eprintln!("If you're getting 'TokenError(Error(InvalidSignature))' it's probably because you're using a new server in /tmp maybe via './run_standalone_temp.sh', thus old credentials for this db won't work, so delete '~/.spacetimedb_client_credentials/db-name-here' so that new creds can be generated on first ever client connect.");
            if let Some(bytes) = body_bytes {
                // Check charset and decode accordingly
                match charset.as_deref() {
                    Some("utf-8" | "utf8") => {
                        match String::from_utf8(bytes) {
                            Ok(body_str) => {
                                panic!("Failed to connect:\n{:?}\nWith decoded body (UTF-8): {}", error, body_str);
                            }
                            Err(from_utf8_error) => {
                                panic!("Failed to connect:\n{:#?}\nFailed to decode body as UTF-8: {}", error, from_utf8_error);
                            }
                        }
                    }
                    Some(other_charset) => {
                        panic!("Failed to connect:\n{:#?}\nUnsupported charset: {}", error, other_charset);
                    }
                    None => {
                        panic!("Failed to connect:\n{:#?}\nNo charset found, thus now assuming UTF-8 body: {:?}", error, String::from_utf8(bytes));
                    }
                }
            } else {
                panic!("Failed to connect: {:#?}", error);
            }
        })
}

// ### Save credentials

fn creds_store() -> credentials::File {
    credentials::File::new("quickstart-chat")
}

/// Our `on_connect` callback: save our credentials to a file.
fn on_connected(_ctx: &DbConnection, _identity: Identity, token: &str) {
    if let Err(e) = creds_store().save(token) {
        eprintln!("Failed to save credentials: {:?}", e);
    }
}

// ### Handle errors and disconnections

/// Our `on_connect_error` callback: print the error, then exit the process.
fn on_connect_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Connection error: {}", err);
    std::process::exit(1);
}

/// Our `on_disconnect` callback: print a note, then exit the process.
fn on_disconnected(_ctx: &ErrorContext, err: Option<Error>) {
    if let Some(err) = err {
        eprintln!("Disconnected with error: {}", err);
        std::process::exit(1);
    } else {
        println!("Disconnected.");
        std::process::exit(0);
    }
}

// ## Register callbacks

/// Register all the callbacks our app will use to respond to database events.
fn register_callbacks(ctx: &DbConnection) {
    // When a new user joins, print a notification.
    ctx.db.user().on_insert(on_user_inserted);

    // When a user's status changes, print a notification.
    ctx.db.user().on_update(on_user_updated);

    // When a new message is received, print it.
    ctx.db.message().on_insert(on_message_inserted);

    // When we fail to set our name, print a warning.
    ctx.reducers.on_set_name(on_name_set);

    // When we fail to send a message, print a warning.
    ctx.reducers.on_send_message(on_message_sent);
}

// ### Notify about new users

/// Our `User::on_insert` callback: if the user is online, print a notification.
fn on_user_inserted(_ctx: &EventContext, user: &User) {
    if user.online {
        println!("User {} connected.", user_name_or_identity(user));
    }
}

fn user_name_or_identity(user: &User) -> String {
    user.name
        .clone()
        .unwrap_or_else(|| user.identity.to_abbreviated_hex().to_string())
}

// ### Notify about updated users

/// Our `User::on_update` callback:
/// print a notification about name and status changes.
fn on_user_updated(_ctx: &EventContext, old: &User, new: &User) {
    if old.name != new.name {
        println!(
            "User {} renamed to {}.",
            user_name_or_identity(old),
            user_name_or_identity(new)
        );
    }
    if old.online && !new.online {
        println!("User {} disconnected.", user_name_or_identity(new));
    }
    if !old.online && new.online {
        println!("User {} connected.", user_name_or_identity(new));
    }
}

// ### Print messages

/// Our `Message::on_insert` callback: print new messages.
fn on_message_inserted(ctx: &EventContext, message: &Message) {
    if !matches!(ctx.event, Event::SubscribeApplied) {
        print_message(ctx, message);
    }
}

fn print_message(ctx: &impl RemoteDbContext, message: &Message) {
    let sender = ctx
        .db()
        .user()
        .identity()
        .find(&message.sender)
        .map(|u| user_name_or_identity(&u))
        .unwrap_or_else(|| "unknown".to_string());
    println!("{}: {}", sender, message.text);
}

// ### Handle reducer failures

/// Our `on_set_name` callback: print a warning if the reducer failed.
fn on_name_set(ctx: &ReducerEventContext, name: &String) {
    if let Status::Failed(err) = &ctx.event.status {
        eprintln!("Failed to change name to {:?}: {}", name, err);
    }
}

/// Our `on_send_message` callback: print a warning if the reducer failed.
fn on_message_sent(ctx: &ReducerEventContext, text: &String) {
    if let Status::Failed(err) = &ctx.event.status {
        eprintln!("Failed to send message {:?}: {}", text, err);
    }
}

// ## Subscribe to tables

/// Register subscriptions for all rows of both tables.
fn subscribe_to_tables(ctx: &DbConnection) {
    ctx.subscription_builder()
        .on_applied(on_sub_applied)
        .on_error(on_sub_error)
        .subscribe(["SELECT * FROM user", "SELECT * FROM message"]);
}

// ### Print past messages in order

/// Our `on_applied` callback:
/// sort all past messages and print them in timestamp order.
fn on_sub_applied(ctx: &SubscriptionEventContext) {
    let mut messages = ctx.db.message().iter().collect::<Vec<_>>();
    messages.sort_by_key(|m| m.sent);
    for message in messages {
        print_message(ctx, &message);
    }
    println!("Fully connected and all subscriptions applied.");
    println!("Use /name to set your name, or type a message!");
}

// ### Notify about failed subscriptions

/// Or `on_error` callback:
/// print the error, then exit the process.
fn on_sub_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Subscription failed: {}", err);
    std::process::exit(1);
}

// ## Handle user input

/// Read each line of standard input, and either set our name or send a message as appropriate.
fn user_input_loop(ctx: &DbConnection) {
    for line in std::io::stdin().lines() {
        if !RUNNING.load(Ordering::SeqCst) {
            println!("Exiting loop gracefully.");
            break;
        }

        let Ok(line) = line else {
            panic!("Failed to read from stdin.");
        };
        if let Some(quitmsg) = line.strip_prefix("/quit") {
            ctx.reducers.send_message(format!("QUIT: {}",quitmsg)).unwrap();
            break;
        } else if let Some(name) = line.strip_prefix("/name ") {
            ctx.reducers.set_name(name.to_string()).unwrap();
        } else {
            ctx.reducers.send_message(line).unwrap();
        }
    }
}
