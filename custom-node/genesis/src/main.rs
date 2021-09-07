use anyhow::Result;
use std::path::Path;
use structopt::StructOpt;
use diem_config::config::NodeConfig;

#[derive(StructOpt)]
#[structopt(
    name = "custom-node",
    about = "Builds a WriteSet transaction to install the custom modules and starts a node",
    rename_all = "kebab-case"
)]
pub struct CustomFramework {
    /// Directory where the node config will be generated. Must not already exist
    #[structopt(long = "node-config-dir")]
    node_config_dir: String,
}
/// Generate a node config under `args.node_config_dir`
fn main() -> Result<()> {
    let args = CustomFramework::from_args();
    custom_node::build_move_sources()?;
    let swarm_config = custom_node::generate_swarm_config(&Path::new(&args.node_config_dir))?;
    // println!()
    let validator_config = NodeConfig::load(swarm_config.config_files[0].clone())?;
    println!("Running a Diem node with custom modules ...");
    diem_node::start(&validator_config, None);
    Ok(())
}
