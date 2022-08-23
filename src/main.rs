use std::time::Duration;
use dialoguer::Input;
use indicatif::ProgressStyle;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::{Deserialize, Serialize};
use web3::types::Address;

type RRTransactionHash = String;

#[derive(Serialize, Deserialize)]
struct RRTransactionDict {
    transactions: Vec<RRTransactionHash>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RRTransaction {
    from: String,
    to: String,
    hash: String,
    block_number: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RRDecodedTransaction {
    from: String,
    to: String,
    hash: RRTransactionHash,
    block_number: u64,
    input: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
}

fn normalize_address(input: &str) -> String {
    input.replace("ronin:", "0x")
}

struct RoninRest {
    address: String,
    host: String,
    client: ClientWithMiddleware,
}

impl RoninRest {
    pub fn new(address: String) -> RoninRest {
        RoninRest {
            address,
            host: "https://ronin.rest".into(),
            client: ClientBuilder::new(reqwest::Client::new()).with(
                RetryTransientMiddleware::new_with_policy(
                    ExponentialBackoff {
                        max_n_retries: 25,
                        min_retry_interval: Duration::from_secs(1),
                        max_retry_interval: Duration::from_secs(15),
                        backoff_exponent: 2
                    }
                )
            ).build(),
        }
    }

    pub async fn sent_transactions(&self) -> RRTransactionDict {
        let data: RRTransactionDict = serde_json::from_str(
            &self.client.get(format!("{}/archive/listSentTransactions/{}", self.host, self.address)).send().await.unwrap().text().await.unwrap()
        ).unwrap();

        data
    }
    pub async fn received_transactions(&self) -> RRTransactionDict {
        let data: RRTransactionDict = serde_json::from_str(
            &self.client.get(format!("{}/archive/listReceivedTransactions/{}", self.host, self.address)).send().await.unwrap().text().await.unwrap()
        ).unwrap();

        data
    }

    pub async fn decode_method(&self, hash: &RRTransactionHash) -> serde_json::Value {
        let data: serde_json::Value = serde_json::from_str(
            &self.client.get(format!("{}/ronin/decodeTransaction/{}", self.host, hash)).send().await.unwrap().text().await.unwrap()
        ).unwrap();

        data
    }

    pub async fn decode_receipt(&self, hash: &RRTransactionHash) -> serde_json::Value {
        let data: serde_json::Value = serde_json::from_str(
            &self.client.get(format!("{}/ronin/decodeTransactionReceipt/{}", self.host, hash)).send().await.unwrap().text().await.unwrap()
        ).unwrap();

        data
    }

    pub async fn transaction(&self, hash: &RRTransactionHash) -> RRTransaction {
        let data: RRTransaction = serde_json::from_str(
            &self.client.get(format!("{}/ronin/getTransaction/{}", self.host, hash)).send().await.unwrap().text().await.unwrap()
        ).unwrap_or(RRTransaction {
            from: "null".to_string(),
            to: "null".to_string(),
            hash: "null".to_string(),
            block_number: 0
        });

        data
    }
}

struct ArgParser {}
impl ArgParser {
    fn parse() -> Vec<String> {
        std::env::args().collect()
    }

    fn split(param: &String) -> Option<String> {

        let args: Vec<String> = ArgParser::parse();

        for arg in args {
            if arg.starts_with(param) {
                let kv: Vec<&str> = arg.split("=").collect();
                if kv.len() == 2 {
                    return Some(kv[1].to_string())
                }
            }
        }

        None
    }
}

#[tokio::main]
async fn main() {

    let use_localhost = match ArgParser::split(&"--localhost".to_string()) {
        None => false,
        Some(_) => true
    };

    let address: String = match ArgParser::split(&"--address".to_string()) {
        None => {
            normalize_address(
                &Input::new()
                    .with_prompt("Please enter your Ronin address")
                    .validate_with(|input: &String| -> Result<(), &str> {
                        let address = normalize_address(input).as_str().parse::<Address>();
                        match address {
                            Ok(_) => Ok(()),
                            Err(_) => Err("Failed to parse your address!")
                        }
                    })
                    .interact()
                    .unwrap()
            )
        },
        Some(passed_address) => {
            let address = normalize_address(&passed_address).as_str().parse::<Address>();
            match address {
                Ok(_) => normalize_address(&passed_address),
                Err(_) => {
                    panic!("Could not parse address!");
                }
            }
        }
    };


    let mut rr = RoninRest::new(address);

    if use_localhost {
        println!(">> !! USING LOCALHOST FOR API CALLS !! <<");
        rr.host = "http://localhost:3000".to_string();
    }

    let mut sent: RRTransactionDict = rr.sent_transactions().await;
    let mut received: RRTransactionDict = rr.received_transactions().await;

    let mut total: Vec<RRTransactionHash> = vec![];

    println!("Sent Transactions: {}\nReceived Transactions: {}\nAddress: {}", sent.transactions.len(), received.transactions.len(), rr.address);

    total.append(&mut sent.transactions);
    total.append(&mut received.transactions);

    total.dedup();

    let progress = indicatif::ProgressBar::new(total.len() as u64);
    progress.set_style(
        ProgressStyle::with_template("{spinner}{bar:100.cyan/blue} {percent:>3}% | [{eta_precise}][{elapsed_precise}] ETA/Elapsed | {pos:>7}/{len:7} {msg}").unwrap()
    );

    println!("Processing: {} transactions", total.len());

    let mut account_data: Vec<RRDecodedTransaction> = vec![];

    for hash in total {
        let tx = rr.transaction(&hash).await;

        if tx.to == "null" && tx.from == "null" {
            println!("Failed to retrieve transaction details: {}", &hash)
        }

        if tx.to != tx.from {
            account_data.push(
                RRDecodedTransaction {
                    from: tx.from,
                    input: Some(rr.decode_method(&hash).await),
                    output: Some(rr.decode_receipt(&hash).await),
                    hash: hash.clone(),
                    to: tx.to,
                    block_number: tx.block_number,
                }
            );
        }

        progress.inc(1);
        progress.set_message(hash);
    }

    progress.set_message("Saving...");

    account_data.sort_by(|a, b| {
        a.block_number.cmp(&b.block_number)
    });

    let output_file_name = format!("{}.json", rr.address);

    std::fs::write(&output_file_name, serde_json::to_string(&account_data).unwrap()).unwrap();

    progress.set_message("FINISH!");

    progress.finish();

    println!("The output was saved to {}", &output_file_name);
}
