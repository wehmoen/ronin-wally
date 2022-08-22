# Ronin Wallet Export

Exports any transaction sent or received by a wallet ordered by block number.

## Usage

```shell

> git clone https://https://github.com/wehmoen/ronin-wally.git
> cd ronin-wally
> cargo build -r 
> ./target/release/wally
```

You will be prompted for your Ronin address then all transactions will be processed.

## Output:

Filename: `YOUR_ADDRESS.json`

```json
[
  {
    "from": "0x...",
    "to": "0x...",
    "hash": "0x...",
    "blockNumber": 12345,
    "input": "ronin.rest/ronin/decodeTransaction",
    "output": "ronin.rest/ronin/decodeTransactionReceipt"
  }
]
```