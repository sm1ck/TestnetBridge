use colored::Colorize;
use ethers::contract::abigen;
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::U256;
use ethers::utils::parse_ether;
use k256::ecdsa::SigningKey;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg32;
use std::error::Error;
use std::process;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::time::{sleep, Duration};

abigen!(
    Quoter,
    r#"[
        function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) public override returns (uint256 amountOut)
    ]"#,
);
abigen!(
    Bridge,
    r#"[
        function swapAndBridge(uint amountIn, uint amountOutMin, uint16 dstChainId, address to, address payable refundAddress, address zroPaymentAddress, bytes calldata adapterParams) external payable
    ]"#,
);

#[derive(Error, Debug)]
pub enum TxError {
    #[error("Value of receipt is none")]
    NoneError,
    #[error("Tx reverted for some reason")]
    TxRevertError,
}
pub struct ChainBook {
    quoter: Address,
    bridge: Address,
    token_in: Address,
    token_out: Address,
    zro: Address,
    scan: String,
    pre_defined_gas: U256,
}

pub type SignWallets = Vec<Wallet<SigningKey>>;
pub const RANDOM_MIN: u32 = 30;
pub const RANDOM_MAX: u32 = 600;
pub const RANDOM_ETH_MIN: f64 = 0.0001;
pub const RANDOM_ETH_MAX: f64 = 0.0002;
pub const RPC: &str = "https://arb1.arbitrum.io/rpc";
pub const IS_SHUFFLE: bool = true;

#[tokio::main]
async fn main() {
    let book: ChainBook = ChainBook {
        quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6"
            .parse::<Address>()
            .unwrap(),
        bridge: "0x0A9f824C05A74F577A536A8A0c673183a872Dff4"
            .parse::<Address>()
            .unwrap(),
        token_in: "0x82af49447d8a07e3bd95bd0d56f35241523fbab1"
            .parse::<Address>()
            .unwrap(),
        token_out: "0xdd69db25f6d620a7bad3023c5d32761d353d3de9"
            .parse::<Address>()
            .unwrap(),
        zro: "0x0000000000000000000000000000000000000000"
            .parse::<Address>()
            .unwrap(),
        scan: String::from("https://arbiscan.io/"),
        pre_defined_gas: U256::from_str("200000").unwrap(),
    };
    let errors: Vec<String> = vec!["insufficient funds for gas".to_string()];
    log("Начало работы!".to_string(), None);
    let mut wallets = match read_privates("./privates.txt") {
        Ok(wallets) => wallets,
        Err(err) => {
            log(format!("Не удалось прочитать приватники: {:?}", err), None);
            process::exit(0x0100);
        }
    };
    if IS_SHUFFLE {
        let mut rng = rand::thread_rng();
        wallets.shuffle(&mut rng);
    }
    retry(wallets, &book, &errors).await;
    log("Завершение работы..".to_string(), None);
}

async fn send_testnet(wallet: &Wallet<SigningKey>, book: &ChainBook) -> Result<(), Box<dyn Error>> {
    let client = Provider::<Http>::try_from(RPC)?;
    let client = Arc::new(client);
    let quoter_address = book.quoter.clone();
    let quote = Quoter::new(quoter_address, Arc::clone(&client));
    let token_in = book.token_in.clone();
    let token_out = book.token_out.clone();
    let fee = 0xbb8;
    let mut rng = Pcg32::from_entropy();
    let amount = rng.gen_range(RANDOM_ETH_MIN..RANDOM_ETH_MAX);
    log(
        format!(
            "Отправка в Görli {} {}..",
            amount.to_string().bold(),
            "ETH".to_string().bold()
        ),
        Some(wallet.address()),
    );
    let amount_in = parse_ether(amount).unwrap();
    let sqrt_price_limit_x96 = U256::from_str("0").unwrap();
    let amount_out = quote
        .quote_exact_input_single(token_in, token_out, fee, amount_in, sqrt_price_limit_x96)
        .call()
        .await?;
    log(
        format!(
            "Quoter: {} {}",
            format_ether_to_float(&amount_out).to_string().bold(),
            "GETH".to_string().bold()
        ),
        Some(wallet.address()),
    );
    let amount_out_min = amount_out * U256::from(94) / U256::from(100);
    let bridge_address = book.bridge.clone();
    let bridge = Bridge::new(bridge_address, Arc::clone(&client));
    let dst_chain_id: u16 = 154;
    let zro_payment_address = book.zro.clone();
    let adapter_params: Bytes = Bytes::from_str("0x").unwrap();
    let bridge_call = bridge.swap_and_bridge(
        amount_in,
        amount_out_min,
        dst_chain_id,
        wallet.address(),
        wallet.address(),
        zro_payment_address,
        adapter_params,
    );
    let provider = Arc::clone(&client);
    let gas_price = provider.get_gas_price().await?;
    let nonce = provider
        .get_transaction_count(wallet.address(), None)
        .await?;
    let chain_id = U64::from(provider.get_chainid().await?.as_u64());
    let amount_in_tx = amount_in * U256::from(120) / U256::from(100);
    let mut tx2: TypedTransaction = build_tx(
        wallet.address(),
        bridge_call.tx.to().unwrap().clone(),
        amount_in_tx,
        bridge_call.tx.data().cloned(),
        nonce,
        chain_id,
        book.pre_defined_gas.clone(),
        gas_price,
        gas_price,
    );
    let gas = provider.estimate_gas(&tx2, None).await?;
    tx2.set_gas(gas);
    let signed = wallet.sign_transaction(&tx2).await?;
    let hash = client.send_raw_transaction(tx2.rlp_signed(&signed)).await?;
    log(
        format!("{}tx/{:?}", book.scan.clone(), hash.tx_hash())
            .underline()
            .to_string(),
        Some(wallet.address()),
    );
    let receipt_opt = hash.await?;
    if receipt_opt.is_none() {
        return Err(TxError::NoneError.into());
    }
    let receipt = receipt_opt.unwrap();
    if receipt.status.unwrap() == 1.into() {
        log(
            "Транзакция подтверждена!".to_string(),
            Some(wallet.address()),
        );
    } else {
        log(
            "Ошибка при подтверждении транзакции:".to_string(),
            Some(wallet.address()),
        );
        println!("{:#?}", receipt);
        return Err(TxError::TxRevertError.into());
    }
    Ok(())
}

async fn retry<'a>(wallets: Vec<Wallet<SigningKey>>, book: &'a ChainBook, errors: &'a Vec<String>) {
    const N_ATTEMPTS: u8 = 10;
    let mut count = 0;
    let size = &wallets.len();
    for wallet in wallets {
        for i in 0..N_ATTEMPTS {
            match send_testnet(&wallet, &book).await {
                Ok(()) => {
                    if count < size - 1 {
                        sleeping(None).await;
                    }
                    break;
                }
                Err(e) => {
                    error(e.as_ref());
                    let str_e = e.to_string();
                    let find =
                        errors
                            .iter()
                            .find_map(|s| if str_e.contains(s) { Some(s) } else { None });
                    if find.is_none() {
                        log(
                            format!(
                                "Повторная отправка: {}{}{}",
                                (i + 1).to_string().green().bold(),
                                "/".to_string().bold(),
                                N_ATTEMPTS.to_string().red().bold()
                            ),
                            Some(wallet.address()),
                        );
                        sleeping(Some(1)).await;
                    } else {
                        break;
                    }
                }
            }
        }
        count += 1;
    }
}

fn build_tx(
    from: H160,
    to: NameOrAddress,
    value: U256,
    data: Option<Bytes>,
    nonce: U256,
    chain_id: U64,
    gas: U256,
    max_priority_fee_per_gas: U256,
    max_fee_per_gas: U256,
) -> TypedTransaction {
    TypedTransaction::Eip1559(Eip1559TransactionRequest {
        from: Some(from),
        to: Some(to),
        value: Some(value),
        data: data,
        nonce: Some(nonce),
        chain_id: Some(chain_id),
        access_list: AccessList::default(),
        gas: Some(gas),
        max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
        max_fee_per_gas: Some(max_fee_per_gas),
    })
}

fn format_ether_to_float(value: &U256) -> f64 {
    value.as_u128() as f64 / 1_000_000_000_000_000_000.0
}

fn from_private(raw: &str) -> Result<Wallet<SigningKey>, WalletError> {
    Wallet::from_str(raw).map_err(|err| err.into())
}

fn read_privates(path: &str) -> Result<SignWallets, WalletError> {
    let content = std::fs::read_to_string(path)?;

    let wallets = content
        .split('\n')
        .flat_map(|line| {
            let line = line.trim();
            let splitted = line.split(':').collect::<Vec<&str>>();
            if splitted.is_empty() {
                return Err("invalid private key".to_string());
            }

            let credentials = splitted[0].trim();

            let wallet: Result<Wallet<SigningKey>, _> = from_private(credentials);

            match wallet {
                Ok(wallet) => Ok(wallet),
                Err(err) => Err(err.to_string()),
            }
        })
        .collect();

    Ok(wallets)
}

fn error(e: &dyn Error) {
    println!("{}", "[ERROR]:".to_string().bold().red());
    println!("{:#?}", e);
}

fn log(text: String, address: Option<Address>) {
    println!(
        "{}{}{}{}",
        "Testnet Bridge => ".bold(),
        address.map_or("".to_string().black(), |addr| format!("{:?}", addr).blue()),
        address.map_or_else(|| "", |_| ": "),
        text
    );
}

async fn sleeping(time: Option<u64>) {
    let sleep_time: u64 = time.map_or_else(
        || {
            let mut rng = Pcg32::from_entropy();
            u64::from(rng.gen_range(RANDOM_MIN..RANDOM_MAX))
        },
        |t| t,
    );
    log(
        format!("Задержка {}{}..", sleep_time.to_string().bold(), "с"),
        None,
    );
    sleep(Duration::from_secs(sleep_time)).await;
}
