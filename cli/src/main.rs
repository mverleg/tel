use ::std::path::PathBuf;

use ::clap::Parser;
use ::clap::Subcommand;
use ::env_logger;

#[derive(Parser, Debug)]
#[command(
    name = "steel",
)]
struct SteelCli {
    #[clap(subcommand)]
    subcommand: SubCmd,
}

#[derive(Parser, Debug)]
#[command(name = "build")]
struct BuildCli {
    /// Path of the file to build
    #[arg(default_value = "./main.steel")]
    pub path: PathBuf,
    /// Print extra debug output
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    Build(BuildCli),
}

#[test]
fn test_cli_args() {
    SteelCli::try_parse_from(&["steel", "build", "-v"]).unwrap();
}

use ::steel::steel_build;
use ::steel::BuildArgs;

fn main() {
    env_logger::init_from_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "warn"));
    let args = SteelCli::parse();
    match args.subcommand {
        SubCmd::Build(build_args) => steel_build(&BuildArgs {
            path: build_args.path,
            verbose: build_args.verbose,
        })
    }.unwrap()  //TODO @mark: do not unwrap
}
