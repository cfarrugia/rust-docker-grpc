use std::sync::atomic::{AtomicUsize, Ordering};
use tonic::{transport::Server, Request, Response, Status};

use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use rand::prelude::*;
use std::time::Instant;
use std::time::Duration;

use clap::App;

#[macro_use]
extern crate clap;

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

#[derive(Debug, Default)]
pub struct MyGreeter {
    server_name : String,
    verbose_ok_responses : bool,
    verbose_err_responses : bool
}

impl MyGreeter {
    fn new(server_name: String, verbose_ok_responses: bool, verbose_err_responses : bool) -> MyGreeter {
        MyGreeter {
            server_name : server_name,
            verbose_ok_responses : verbose_ok_responses,
            verbose_err_responses : verbose_err_responses
        }
    }
}


#[tonic::async_trait]
impl Greeter for MyGreeter {
    
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {

        let req : HelloRequest = request.into_inner();

        if self.verbose_ok_responses {
            println!("Got a request from {}, {} : {:?}", req.client_name, req.thread_name, req.message);
        }

        let reply = hello_world::HelloReply {
            server_name : self.server_name.clone(),
            message: format!("Hello {} ! This is server {}", req.client_name, self.server_name.clone()).into(),

        };


        GLOBAL_OK_COUNT.fetch_add(1, Ordering::SeqCst);

        Ok(Response::new(reply))
    }
}


static GLOBAL_OK_COUNT: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    let yaml = load_yaml!("srv.yml");
    let matches = App::from_yaml(yaml).get_matches();

    // Load all the clap arguments
    let listenhost = matches.value_of("listenhost").unwrap();
    let listenport = matches.value_of("listenport").and_then(|s| s.parse::<u16>().ok()).unwrap();
    let delay_s_between_metric_report = matches.value_of("delay_s_between_metric_report").and_then(|s| s.parse::<u64>().ok()).unwrap();
    let verbose_ok_responses = matches.value_of("verbose_ok_responses").and_then(|s| s.parse::<bool>().ok()).unwrap();
    let verbose_err_responses = matches.value_of("verbose_err_responses").and_then(|s| s.parse::<bool>().ok()).unwrap();
    let show_metrics = matches.value_of("show_metrics").and_then(|s| s.parse::<bool>().ok()).unwrap();



    let addr = format!("{}:{}", listenhost, listenport);
    let addr = addr.parse()?;

    let serv_name = generate_server_name();

    println!("Starting server with name: {}", serv_name);
    let greeter = MyGreeter::new(serv_name, verbose_ok_responses, verbose_err_responses);

    let report_future = tokio::spawn(async move {

        if show_metrics {
            let mut start = Instant::now();

            loop {
                // Delay 5 sec
                tokio::time::sleep(Duration::from_secs(delay_s_between_metric_report)).await;
    
    
                let elapsed = start.elapsed().as_millis() as f64;
                let avg_ok = (GLOBAL_OK_COUNT.swap(0, Ordering::SeqCst) * 1000) as f64;
                let avg_ok = (avg_ok / elapsed) as u32;
    
                start = Instant::now();
    
                println!("Avg Req/S. Success: {}. ",  avg_ok);
            }
        }
    });


    let serv_future = Server::builder()
        .add_service(GreeterServer::new(greeter))
        .serve(addr);
    
    let (report_result, serv_result) = tokio::join!(report_future, serv_future);

    Ok(())
}

fn generate_server_name() -> String {
    format!("Server-{}", thread_rng().next_u32())
}