use backoff::SystemClock;
use std::ops::Deref;
use std::time::Duration;

use backoff::Clock;
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use backoff::future::retry_notify;

use futures::FutureExt;
use hello_world::greeter_client::GreeterClient;
use hello_world::HelloRequest;
use rand::prelude::*;
use tonic::Response;
use tonic::Status;
use tonic::transport::Channel;
use std::time::Instant;
use ginepro::LoadBalancedChannel;

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

use std::sync::atomic::{AtomicUsize, Ordering};

use clap::App;

#[macro_use]
extern crate clap;



static GLOBAL_OK_COUNT: AtomicUsize = AtomicUsize::new(0);
static GLOBAL_RETRY_COUNT: AtomicUsize = AtomicUsize::new(0);
static GLOBAL_ERR_COUNT: AtomicUsize = AtomicUsize::new(0);




#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // The YAML file is found relative to the current file, similar to how modules are found
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    // Load all the clap arguments
    let serverhost = matches.value_of("serverhost").unwrap();
    let serverport = matches.value_of("serverport").and_then(|s| s.parse::<u16>().ok()).unwrap();
    let number_of_thread : u32 = matches.value_of("number_of_threads").and_then(|s| s.parse::<u32>().ok()).unwrap();
    let delay_ms_between_calls = matches.value_of("delay_ms_between_calls").and_then(|s| s.parse::<u64>().ok()).unwrap();
    let delay_s_between_metric_report = matches.value_of("delay_s_between_metric_report").and_then(|s| s.parse::<u64>().ok()).unwrap();
    let verbose_ok_responses = matches.value_of("verbose_ok_responses").and_then(|s| s.parse::<bool>().ok()).unwrap();
    let verbose_err_responses = matches.value_of("verbose_err_responses").and_then(|s| s.parse::<bool>().ok()).unwrap();
    let show_metrics = matches.value_of("show_metrics").and_then(|s| s.parse::<bool>().ok()).unwrap();


    let load_balanced_channel = LoadBalancedChannel::builder((serverhost, serverport))
      .await
      .expect("Failed to initialise the DNS resolver.")
      .dns_probe_interval(std::time::Duration::from_secs(5))
      .channel();

      
    // let cc = load_balanced_channel.clone();
    // let l  = Channel::from(cc);
    


    let client = GreeterClient::new(load_balanced_channel);
    let client_name = generate_client_name();



    // Hold the handles here.
    let mut handles = Vec::new();

    if show_metrics {
        let h = tokio::spawn(async move {

            let mut start = Instant::now();

            loop {
                // Delay 5 sec
                tokio::time::sleep(Duration::from_secs(delay_s_between_metric_report)).await;


                let elapsed = start.elapsed().as_millis() as f64;
                let avg_ok = (GLOBAL_OK_COUNT.swap(0, Ordering::SeqCst) * 1000) as f64;
                let avg_ok = (avg_ok / elapsed) as u32;

                let total_failed = GLOBAL_ERR_COUNT.swap(0, Ordering::SeqCst);
                let total_retry = GLOBAL_RETRY_COUNT.swap(0, Ordering::SeqCst);

                start = Instant::now();

                println!("Avg Success Req/S. {}. Total Failed: {}, Total Retry: {}",  avg_ok, total_failed, total_retry);
            }
        });

        handles.push(h);
        
    }

    for _ in 0 .. number_of_thread {
        
        let c = client.clone();
        let c_name = client_name.clone();

        handles.push(tokio::spawn(async move {
    
        let t_name = generate_thread_name();

        loop {

            
            // Wait for delay.
            if delay_ms_between_calls > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms_between_calls)).await;
            }

            let INITIAL_INTERVAL_MILLIS = 200;
            let RANDOMIZATION_FACTOR = 0.5;
            let MULTIPLIER = 2.0;
            let MAX_ELAPSED_TIME_SECONDS = 5;
            let MAX_INTERVAL_MILLIS = 1500;

            //
            let mut exp = ExponentialBackoff::default();
            exp.current_interval = Duration::from_millis(INITIAL_INTERVAL_MILLIS);
            exp.initial_interval = Duration::from_millis(INITIAL_INTERVAL_MILLIS);
            exp.randomization_factor = RANDOMIZATION_FACTOR;
            exp.multiplier = MULTIPLIER;
            exp.max_interval = Duration::from_secs(MAX_INTERVAL_MILLIS);
            exp.max_elapsed_time = Some(Duration::new(MAX_ELAPSED_TIME_SECONDS, 0));
            exp.reset();
            


            // Notify when retrying! 
            let notify = |err, dur| { 
                let _ = GLOBAL_RETRY_COUNT.fetch_add(1, Ordering::SeqCst);
                    if verbose_err_responses {
                        println!("Error occurred! Retrying! ERR={:?}", err); 
                    }

            };

            // This is the operation to be carried out.
            let op = || async 
            {
                let mut c2 = c.clone();
                let c2_name = c_name.clone();
                let t2_name = t_name.clone();


                let request = tonic::Request::new(HelloRequest {
                    client_name : c2_name,
                    thread_name: t2_name,
                    message: "Tonic".into(),
                });

                Ok(c2.say_hello(request).await?)
            };

            match retry_notify(exp, op ,notify).await {
                Ok(resp) => {
                    let _ = GLOBAL_OK_COUNT.fetch_add(1, Ordering::SeqCst);
                    if verbose_ok_responses {
                        println!("RESPONSE={:?}", resp); 
                    }
                },
                Err (e) => {
                    let _ = GLOBAL_ERR_COUNT.fetch_add(1, Ordering::SeqCst);
                    if verbose_err_responses {
                        println!("Too many retries! Error={:?}", e);    
                    }
                } 
            } 


           
        }
        }));
    }
    
    // Fire them all!
    futures::future::join_all(handles).await;
    

    Ok(())
}


// async fn retry_notify2<I, E, Fn, Fut>(
//     operation: Fn
// ) -> Result<I, tonic::Status>
// where
//     Fn: FnMut() -> Fut,
//     Fut: futures::Future<Output = Result<I, tonic::Status>> 
// {
    
//     // Notify when retrying! 
//     let notify = |err, dur| { 
//         let _ = GLOBAL_RETRY_COUNT.fetch_add(1, Ordering::SeqCst);
//             if true {
//                 println!("Error occurred! Retrying! ERR={:?}", err); 
//             }

//     };

//     retry_notify(ExponentialBackoff::default(), operation ,notify).await



// }

// // This method is a generic way of retrying requests to tonic clients.
// async fn retry_client_operation<F, T>(future: F) -> Result<tonic::Response<T>, tonic::Status>
// where
//     F: std::future::Future<Output = Z>  + Send + Clone + 'static,
//     Z: Result<tonic::Response<T>, tonic::Status>,
//     T: Send + 'static + Clone,
// {

//     let mut retries: i32 = 3;
//     let mut wait: u64 = 1;
    
    
//     loop {
//         match future.shared().await {            
//             Err(e) => { 
//                 match e.code() {
//                     tonic::Code::Unknown |
//                     tonic::Code::ResourceExhausted |
//                     tonic::Code::Unavailable |
//                     tonic::Code::Aborted => {
//                         // If we have any of these codes we retry. 
//                         if retries > 0 {
//                             retries -= 1;
//                             tokio::time::sleep(Duration::from_secs(wait)).await;
//                             wait *= 2;    
//                         }
//                     },
//                     _ => { return Err(e); } // If not a retryable error, return immediately.
//                 }
//             }, 
//             Ok(o) => {return Ok(o); }
//         }
//     }
// }

fn generate_client_name() -> String {
    format!("Client-{}", thread_rng().next_u32() as u8)
}

fn generate_thread_name() -> String {
    format!("Thread-{}", thread_rng().next_u32() as u8)
}