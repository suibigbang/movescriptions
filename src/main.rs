use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};
use futures::StreamExt;
use clap::{Parser, Subcommand};
use move_core_types::language_storage::StructTag;
use shared_crypto::intent::Intent;
use sui_keys::keystore::{AccountKeystore, InMemKeystore};
use sui_sdk::rpc_types::{EventFilter, SuiExecutionStatus, SuiTransactionBlockEffects, SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions};
use sui_sdk::{SuiClient, SuiClientBuilder};
use sui_sdk::types::base_types::{ObjectID, SequenceNumber, SuiAddress};
use sui_sdk::types::crypto::SignatureScheme;
use sui_sdk::types::{Identifier, SUI_CLOCK_OBJECT_ID, SUI_CLOCK_OBJECT_SHARED_VERSION};
use sui_sdk::types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_sdk::types::transaction::{Argument, Command, ObjectArg, Transaction, TransactionData};



#[derive(Parser)]
#[command(name = "movescription")]
#[command(bin_name = "movescription")]
struct MovescriptionCli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Mint {
        #[arg(short = 'k')]
        mnemonic_path: PathBuf,
        #[arg(short = 't', long)]
        tick: String,
        #[arg(short = 'a', long)]
        tick_address: String,
        #[arg(short = 'f')]
        mint_fee: u64,
        #[arg(long)]
        ws: Option<String>,
        #[arg(long)]
        http: Option<String>,
    },
}

const SUI_MAINNET: &str = "https://rpc-mainnet.suiscan.xyz:443";
//"wss://sui-rpc-mainnet.testnet-pride.com/websocket"
const SUI_MAIN_WS: &str ="wss://rpc-mainnet.suiscan.xyz:443/websocket";
// const SUI_MAIN_WS: &str ="wss://sui1mainnet-ws.chainode.tech:443";
// const SUI_MAINNET: &str = "https://sui1mainnet-ws.chainode.tech:443";

const MOVESCRIPTIONS_ADDRESS: &str = "0x830fe26674dc638af7c3d84030e2575f44a2bdc1baa1f4757cfe010a4b106b6a";
const NEW_EPOCH_STRUCT_TAG: &str = "0x830fe26674dc638af7c3d84030e2575f44a2bdc1baa1f4757cfe010a4b106b6a::movescription::NewEpoch";
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli: MovescriptionCli = MovescriptionCli::parse();
    match cli.command {
        Commands::Mint { tick, tick_address, mnemonic_path, mint_fee,ws,http } => {
                        let mnemonic = std::fs::read_to_string(mnemonic_path.as_path())?;
            let mut k = InMemKeystore::default();
            let address = k.import_from_mnemonic(mnemonic.as_str(), SignatureScheme::ED25519, None)?;
            println!("use address {}", address);
            loop {
                let sui_mainnet = SuiClientBuilder::default()
                    .ws_url(ws.as_deref().unwrap_or(SUI_MAIN_WS)).ws_ping_interval(Duration::from_secs(10))
                    .build(http.as_deref().unwrap_or(SUI_MAINNET)).await?;
                println!("Sui mainnet version: {}", sui_mainnet.api_version());

                if let Err(e) = start(sui_mainnet, &k, address, tick.clone(), tick_address.clone(), mint_fee).await {
                    println!("{:?}", e);
                }
            }
        }
    };
}

async fn start(sui_mainnet: SuiClient, k: &impl AccountKeystore, address: SuiAddress, tick: String, tick_address: String, mint_fee: u64)->anyhow::Result<()> {
    // then listen to epoch to mint
    let mut new_epoch_event = sui_mainnet.event_api().subscribe_event(EventFilter::MoveEventType(StructTag::from_str(NEW_EPOCH_STRUCT_TAG)?)).await?;
    while let Some(r) = StreamExt::next(&mut new_epoch_event).await {
        match r {
            Err(e) => {
                println!("event error: {:?}",e );
            }
            Ok(event) => {
                let value = event.parsed_json;
                let epoch: u64 = value.get("epoch").unwrap().as_str().unwrap().parse()?;
                println!("new epoch: {:?}", epoch);

                let time = Instant::now();
                let txn_response = mint(k, &sui_mainnet, address, tick.clone(), tick_address.clone(), mint_fee).await?;

                let _err = match txn_response.effects {
                    None => Some("None txn".to_string()),
                    Some(tx) =>{
                        match tx {
                            SuiTransactionBlockEffects::V1(data) => {
                                match data.status {
                                    SuiExecutionStatus::Success => {None}
                                    SuiExecutionStatus::Failure { error } => {Some(error)}
                                }
                            }
                        }
                    }
                };

                println!("mint: {}, consume: {:?}, status: {:?}", txn_response.digest, time.elapsed(), txn_response.errors);
            }
        }
    }
    Ok(())
}

async fn mint(key_store: &impl AccountKeystore, sui_mainnet: &SuiClient,  address: SuiAddress,tick: String, tick_address: String, mint_fee: u64) -> anyhow::Result<SuiTransactionBlockResponse> {
    let coins = sui_mainnet
        .coin_read_api()
        .get_coins(address, None, None, Some(1))
        .await?;
    let coin = coins.data.into_iter().next().unwrap();
    let mut ptb = ProgrammableTransactionBuilder::new();
    let tick_record = ObjectID::from_hex_literal(tick_address.as_str())?;

    //let response = sui_mainnet.read_api().get_object_with_options(tick_record, SuiObjectDataOptions::new()).await?.data.unwrap();
    let tick_object_version = 45_771_756;
    let tick_record = ptb.obj(ObjectArg::SharedObject { id: tick_record, initial_shared_version: SequenceNumber::from_u64(tick_object_version), mutable: true })?;
    let tick_name = ptb.pure(tick)?;
    let mint_fee = ptb.pure(mint_fee)?;
    let clock = ptb.obj(ObjectArg::SharedObject {
        id: SUI_CLOCK_OBJECT_ID,
        initial_shared_version: SUI_CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;
    let fee = ptb.command(Command::SplitCoins(Argument::GasCoin, vec![mint_fee]));
    let movescriptions = ObjectID::from_hex_literal(MOVESCRIPTIONS_ADDRESS)?;

    ptb.command(Command::move_call(movescriptions,
                                   Identifier::from_str("movescription").unwrap(), Identifier::from_str("mint").unwrap(),
                                   vec![], vec![tick_record, tick_name, fee, clock]));
    let tx = ptb.finish();
    let gas_price = 750;
    //let gas_price = sui_mainnet.read_api().get_reference_gas_price().await?;

    // let gas_used = {
    //     let tx_data = TransactionData::new_programmable(
    //         address,
    //         vec![coin.object_ref()],
    //         tx.clone(),
    //         1_000_000_000,
    //         gas_price,
    //     );
    //     let resp = sui_mainnet.read_api().dry_run_transaction_block(tx_data).await?;
    //     match resp.effects {
    //         SuiTransactionBlockEffects::V1(t) => {t.gas_used.gas_used()}
    //     }
    // };


    // create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        address,
        vec![coin.object_ref()],
        tx.clone(),
        1_000_000_000,
        gas_price,
    );

    let signature = key_store.sign_secure(&address, &tx_data, Intent::sui_transaction())?;

    let transaction_response = sui_mainnet
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::default(),
            None,
        )
        .await?;
    Ok(transaction_response)
}