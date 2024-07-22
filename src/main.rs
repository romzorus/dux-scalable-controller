// The controller is handling :
    // - CLI
    // - TaskList parsing
    // - HostList parsing
    // - Assignments production
    // - Results display
use std::{collections::HashMap, str::FromStr};
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;
use std::error::Error;
use std::process::exit;
use log::{debug, error, log_enabled, info, Level, warn};
use env_logger::Env;

use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{
        BasicConsumeArguments, BasicPublishArguments, QueueBindArguments, QueueDeclareArguments, BasicGetArguments,
    },
    connection::{Connection, OpenConnectionArguments},
    consumer::DefaultConsumer,
    BasicProperties,
};
use pem::Pem;
use tokio::time::Duration;
use tracing_subscriber::{fmt, prelude::*};
use tracing_subscriber::filter::EnvFilter;
use simple_crypt::encrypt;
use simple_crypt::decrypt;

use duxcore::prelude::*;

mod cliargs;
mod conf;

use crate::cliargs::{parse_cli_args_scalable_controller, CliArgsScalableController};
use crate::conf::DuxConfigScalableController;

#[tokio::main]
async fn main() {
    let env = Env::default()
    .filter_or("INFO", "info");

    env_logger::init_from_env(env);

    welcome_message_scalable_controller();

    // Parse the CLI arguments
    let cliargs: CliArgsScalableController = parse_cli_args_scalable_controller().unwrap();

    // Get the configuration
    let conf = DuxConfigScalableController::from(cliargs.conf).expect("Unable to determine configuration. Abort.");
    
    // Build a HostList
    let hostlist = hostlist_parser(
        hostlist_get_from_file(&cliargs.hostlist.as_ref().unwrap())
    );

    if hostlist_get_all_hosts(&hostlist).is_none() {
        warn!("No hosts in given list ({})", &cliargs.hostlist.as_ref().unwrap());
        exit(0);
    }

    // Build Assignments (an Assignment is basically a Host associated to a TaskList)
    //  -> Initialization of CorrelationId (not really required for all-in-one mode)
    let mut correlationid = CorrelationIdGenerator::new();
    match correlationid.init() {
        Ok(_) => {}
        Err(e) => {
            error!("Error: failure to initialize CorrelationId : {:?}", e);
            exit(1);
        }
    }
    //  -> Actual build of Assignments
    let mut assignmentlist: Vec<Assignment> = Vec::new();
    let mut correlationidlist: Vec<String> = Vec::new();

    // This unwrap is safe since we checked before that the list is not empty.
    for host in hostlist.hosts.unwrap() {

        let authmode = match &cliargs.key {
            Some(privatekeypath) => {
                let mut keyfile = match File::open(privatekeypath) {
                    Ok(f) => { f }
                    Err(e) => {
                        error!("Unable to open SSH key file : {}", e);
                        exit(1);
                    }
                };
            
                let mut buffer = String::new();
            
                match keyfile.read_to_string(&mut buffer) {
                    Ok(bytes_number) => {
                        if bytes_number == 0 {
                            error!("No byte read from SSH key file. Is it really empty ?");
                            exit(1);
                        } else {
                            let pem_encoded_key = Pem::from_str(buffer.as_str()).unwrap();

                            Ssh2AuthMode::KeyMemory((
                                cliargs.user.clone().unwrap(),
                                pem_encoded_key
                            ))
                        }
                    }
                    Err(e) => {
                        error!("Unable to read SSH key content : {}", e);
                        exit(1);
                    }
                }
            }
            None => {
                // No SSH key given as argument, trying with password if it is given
                match cliargs.password.clone() {
                    Some(pwd) => {
                        Ssh2AuthMode::UsernamePassword(
                            Credentials::from(cliargs.user.clone().unwrap(), pwd)
                        )
                    }
                    None => {
                        error!("No SSH key or password to connect to remote host.");
                        exit(1);
                    }
                }
            }
        };

        // Build a TaskList (YAML is assumed for now)
        let tasklist = tasklist_parser(
            tasklist_get_from_file(&cliargs.tasklist.as_ref().unwrap()),
            &host
            );
        
        if tasklist.tasks.is_empty() {
            warn!("No task in given list ({})", &cliargs.tasklist.as_ref().unwrap());
            exit(0);
        }

        // This unwrap() is safe because initialization is checked before.
        // SSH2 is assumed for now.
        let correlationid = correlationid.get_new_value().unwrap();
        correlationidlist.push(correlationid.clone());
        
        assignmentlist.push(Assignment::from(
            correlationid,
            RunningMode::Apply,
            host.address.clone(),
            HostHandlingInfo::from(ConnectionMode::Ssh2, host.address.clone(), ConnectionDetails::Ssh2(Ssh2ConnectionDetails::from(host.address.clone(), authmode))),
            HashMap::new(),
            tasklist.clone(),
            ChangeList::new(),
            ResultList::new(),
            AssignmentFinalStatus::Unset
        ));
    }
 
    let mut resultslist: Vec<Assignment> = Vec::new();

    // Now, Assignments need to be sent to Message Broker (MB).
    // After this, "consume" the results from MB and push them in 'resultslist'.
    // To make sure we have all results, establish a list of CorrelationId and
    // use it as a checklist.

    // let correlationIdList = ...

    // resultslist.push(assignment);

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .try_init()
        .ok();

    let connection = Connection::open(&OpenConnectionArguments::new(
        conf.rabbitmq.rmq_address.as_str(),
        conf.rabbitmq.rmq_port,
        conf.rabbitmq.rmq_username.as_str(),
        conf.rabbitmq.rmq_password.as_str(),
    ))
    .await
    .unwrap();

    // open a channel on the connection
    let channel = connection.open_channel(None).await.unwrap();
    channel
        .register_callback(DefaultChannelCallback)
        .await
        .unwrap();

    // declare a durable queue
    let (queue_name, _, _) = channel
        .queue_declare(QueueDeclareArguments::durable_client_named(
            "assignments",
        ))
        .await
        .unwrap()
        .unwrap();

    // bind the queue to exchange
    let routing_key = "assignments";
    let exchange_name = "amq.direct";
    channel
        .queue_bind(QueueBindArguments::new(
            &queue_name,
            exchange_name,
            routing_key,
        ))
        .await
        .unwrap();

    // publish message  
    for assignment in assignmentlist.iter() {

        let content = serde_json::to_string(&assignment).unwrap().into_bytes();

        // Encrypt data
        let encrypted_content = encrypt(&content, conf.encryption.password.as_bytes()).unwrap();

        // create arguments for basic_publish
        let args = BasicPublishArguments::new(exchange_name, routing_key);

        channel
            .basic_publish(BasicProperties::default(), encrypted_content, args)
            .await
            .unwrap();

        info!("{} : assignment sent to message broker", assignment.correlationid);
    }
    println!("");
    // Fetch a Result
    let args = BasicGetArguments::new("results")
        .no_ack(true)
        .finish();
    
    loop {
        tokio::time::sleep(Duration::from_millis(REFRESH_INTERVAL_MILLI_SECONDS)).await;

        match channel.basic_get(args.clone()).await {
            Ok(content) => {
                match content {
                    Some((_, _, raw_message)) => {
                        // Decrypt data
                        let decrypted_serialized_result = decrypt(&raw_message, conf.encryption.password.as_bytes()).unwrap();

                        let assignment_result: Assignment = serde_json::from_str(&String::from_utf8_lossy(&decrypted_serialized_result)).unwrap();

                        match correlationidlist.iter().position(|x| (*x).eq(&assignment_result.correlationid)) {
                            Some(index) => { correlationidlist.remove(index); }
                            None => {} // TODO: handle this case : a result is in the queue but it doesn't match the assignments run this time
                        }
                        

                        info!("{} : assignment result received", assignment_result.correlationid);

                        display_output(assignment_result.clone());
                        resultslist.push(assignment_result);

                        if correlationidlist.is_empty() {
                            break;
                        }
                    }
                    None => {}
                }                        
            }
            Err(_) => {}
        }        
    }


    channel.close().await.unwrap();
    connection.close().await.unwrap();

}

pub fn welcome_message_scalable_controller() {
    println!(
        r"
    ██████╗ ██╗   ██╗██╗  ██╗
    ██╔══██╗██║   ██║╚═███╔═╝
    ██║  ██║██║   ██║  ███║ 
    ██████╔╝╚██████╔╝██╔╝ ██╗
    ╚═════╝  ╚═════╝ ╚═╝  ╚═╝
    🅲🅾🅽🆃🆁🅾🅻🅻🅴🆁
"
    );
}
