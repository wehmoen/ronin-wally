use dialoguer::Input;
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

#[derive(Serialize, Deserialize)]
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
                    ExponentialBackoff::builder().build_with_max_retries(15)
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
        ).unwrap();

        data
    }
}

#[tokio::main]
async fn main() {

    let address: String = normalize_address(
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
    );


    let rr = RoninRest::new(address);

    let mut sent: RRTransactionDict = rr.sent_transactions().await;
    let mut received: RRTransactionDict = rr.received_transactions().await;

    let mut total: Vec<RRTransactionHash> = vec![];

    println!("Sent Transactions: {}\nReceived Transactions: {}", sent.transactions.len(), received.transactions.len());

    total.append(&mut sent.transactions);
    total.append(&mut received.transactions);

    total.dedup();

    println!("Processing: {} valid transactions", total.len());

    let mut account_data: Vec<RRDecodedTransaction> = vec![];

    for hash in total {
        let tx = rr.transaction(&hash).await;

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
        println!("Completed: {}", &hash);
    }

    account_data.sort_by(|a, b| {
        a.block_number.cmp(&b.block_number)
    });

    std::fs::write(format!("{}.json", rr.address), serde_json::to_string(&account_data).unwrap()).unwrap();
}
