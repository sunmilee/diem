use anyhow::{Context, Result, anyhow};
use diem_config::config::NodeConfig;
use diem_crypto::PrivateKey;
use diem_sdk::{
    client::BlockingClient,
    transaction_builder::{Currency, TransactionFactory},
    types::{LocalAccount, transaction::{Module, TransactionPayload}, account_address::AccountAddress},
};
use move_command_line_common::files::MOVE_EXTENSION;
use diem_types::{
    account_config, chain_id::ChainId, transaction::authenticator::AuthenticationKey, account_state_blob::AccountStateBlob,
    account_state::AccountState,
};
use anyhow::{bail, format_err};
use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    convert::TryFrom,
};
use diem_temppath::TempPath;
use generate_key::load_key;
use structopt::StructOpt;
use transaction_builder::framework::{
    encode_create_regular_account_script_function, encode_mint_coin_script_function,
};
use move_binary_format::{normalized, file_format::CompiledModule};

#[derive(Debug, StructOpt)]
#[structopt(about = "Fancy coin app")]
pub struct SampleApp {
    /// Directory where the node config lives.
    #[structopt(long = "node-config-dir")]
    node_config_dir: PathBuf,
    #[structopt(long = "account-key-path")]
    account_key_path: PathBuf,
}

fn main() -> Result<()> {
    let args = SampleApp::from_args();
    let config_path = args.node_config_dir.join("0").join("node.yaml");
    let config = NodeConfig::load(&config_path)
        .with_context(|| format!("Failed to load NodeConfig from file: {:?}", config_path))?;
    let root_key_path = args.node_config_dir.join("mint.key");
    let root_account_key = load_key(root_key_path);
    let new_account_key = load_key(args.account_key_path);
    let json_rpc_url = format!("http://0.0.0.0:{}", config.json_rpc.address.port());
    let factory = TransactionFactory::new(ChainId::test());
    println!("Connecting to {}...", json_rpc_url);

    let client = BlockingClient::new(json_rpc_url.clone());
    let root_seq_num = client
        .get_account(account_config::diem_root_address())?
        .into_inner()
        .unwrap()
        .sequence_number;
    let mut root_account = LocalAccount::new(
        account_config::diem_root_address(),
        root_account_key,
        root_seq_num,
    );
    let mut new_account = LocalAccount::new(
        AuthenticationKey::ed25519(&new_account_key.public_key()).derived_address(),
        new_account_key,
        0,
    );
    println!("======new account {}", new_account.address());

    // Create a new account.
    print!("Create a new account...");
    let create_new_account_txn = root_account.sign_with_transaction_builder(
        TransactionFactory::new(ChainId::test()).payload(
            encode_create_regular_account_script_function(
                Currency::XUS.type_tag(),
                new_account.address(),
                new_account.authentication_key().prefix().to_vec(),
            ),
        ),
    );
    send(&client, create_new_account_txn)?;
    println!("Success!");

    // Mint a coin to the newly created account.
    print!("Mint a coin to the new account...");
    let mint_coin_txn = new_account.sign_with_transaction_builder(
        TransactionFactory::new(ChainId::test()).payload(encode_mint_coin_script_function(100)),
    );
    send(&client, mint_coin_txn)?;
    println!("Success!");

    // ================= Send a module transaction ========================
    print!("Add a module to user account...");

    // Get the path to the Move stdlib sources
    let move_stdlib_dir = move_stdlib::move_stdlib_modules_full_path();
    let diem_framework_dir = diem_framework::diem_stdlib_modules_full_path();

    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("move/src/modules/user_modules/SampleUserModule.move")
        .canonicalize()?;
    println!("=======module path: {}", module_path.display());
    let copied_module_path =
        copy_file_with_sender_address(&module_path, new_account.address()).unwrap();
    println!("=======copied_module_path: {}", copied_module_path.display());
    let unwrapped_module_path = copied_module_path.to_str().unwrap();
    println!("=======unwrapped_module_path: {}", unwrapped_module_path);
    let compiled_module = compile_program(
        unwrapped_module_path,
        &[move_stdlib_dir.as_str(), diem_framework_dir.as_str()],
    )?;

    let publish_txn = new_account.sign_with_transaction_builder(
        factory.payload(TransactionPayload::Module(Module::new(compiled_module))),
    );

    send(&client, publish_txn)?;
    println!("Success!");

    // ================= Get modules in the account  ========================

    let account_state_blob: AccountStateBlob = {
        let blob = client
            .get_account_state_with_proof(new_account.address(), None, None)
            .unwrap()
            .into_inner()
            .blob
            .ok_or_else(|| anyhow::anyhow!("missing account state blob"))?;
        bcs::from_bytes(&blob)?
    };
    let account_state = AccountState::try_from(&account_state_blob)?;
    // println!("account_address {}", account_state.get_account_address().unwrap().unwrap());
    let mut modules = vec![];
    for module_bytes in account_state.get_modules() {
        modules.push(
            normalized::Module::new(&CompiledModule::deserialize(module_bytes)
                .map_err(|e| anyhow!("Failure deserializing module: {:?}", e))?),
        );
    };
    println!("move modules length: {}", modules.len());
    println!("move modules name: {}", modules[0].name);

    // // This fails because a coin resource has already been published to the new account
    // // Mint a coin to the newly created account.
    // println!("Mint another coin to the new account (this should fail)...");
    // let mint_another_coin_txn = new_account.sign_with_transaction_builder(
    //     TransactionFactory::new(ChainId::test()).payload(encode_mint_coin_script_function(42)),
    // );
    // send(&client, mint_another_coin_txn)?;
    // Should not reach here
    // println!("Success!");
    Ok(())
}

/// Send a transaction to the blockchain through the blocking client.
fn send(client: &BlockingClient, tx: diem_types::transaction::SignedTransaction) -> Result<()> {
    use diem_json_rpc_types::views::VMStatusView;

    client.submit(&tx)?;
    assert_eq!(
        client
            .wait_for_signed_transaction(&tx, Some(std::time::Duration::from_secs(60)), None)?
            .into_inner()
            .vm_status,
        VMStatusView::Executed,
    );
    Ok(())
}


/// Compile Move program
pub fn compile_program(file_path: &str, dependency_paths: &[&str]) -> Result<Vec<u8>> {
    let tmp_output_dir = TempPath::new();
    tmp_output_dir
        .create_as_dir()
        .expect("error creating temporary output directory");
    let tmp_output_path = tmp_output_dir.as_ref().display().to_string();

    let mut command = Command::new("cargo");
    command
        .args(&["run", "-p", "move-lang", "--bin", "move-build", "--"])
        .arg(file_path)
        .args(&["-o", &tmp_output_path]);

    for dep in dependency_paths {
        command.args(&["-d", dep]);
    }
    for (name, addr) in diem_framework::diem_framework_named_addresses() {
        command.args(&["-a", &format!("{}=0x{:#X}", name, addr)]);
    }

    let output = command.output()?;
    if !output.status.success() {
        return Err(format_err!("compilation failed"));
    }

    let mut output_files = walkdir::WalkDir::new(tmp_output_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            e.file_type().is_file()
                && path
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext == "mv")
                .unwrap_or(false)
        })
        .filter_map(|e| e.path().to_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    if output_files.is_empty() {
        bail!("compiler failed to produce an output file")
    }

    let compiled_program = if output_files.len() != 1 {
        bail!("compiler output has more than one file")
    } else {
        fs::read(output_files.pop().unwrap())?
    };

    Ok(compiled_program)
}


fn copy_file_with_sender_address(file_path: &Path, sender: AccountAddress) -> io::Result<PathBuf> {
    let tmp_source_path = TempPath::new().as_ref().with_extension(MOVE_EXTENSION);
    let mut tmp_source_file = std::fs::File::create(tmp_source_path.clone())?;
    let mut code = fs::read_to_string(file_path)?;
    code = code.replace("{{sender}}", &format!("0x{}", sender));
    writeln!(tmp_source_file, "{}", code)?;
    Ok(tmp_source_path)
}
