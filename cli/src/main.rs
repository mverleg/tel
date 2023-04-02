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

    // /// Duration the cache should be valid for, e.g. "30 min" or "1 day -1 hour".
    // #[arg(value_parser = parse_dur, short = 'd', long = "duration", default_value = "15 min")]
    // pub duration: Duration,
    // #[arg(
    // short = 'k',
    // long = "key",
    // default_value = "%{pwd}_%{env}_%{cmd}.cache",
    // help = "The key to use for the cache. Can use %{pwd}, %{env} and %{cmd} placeholders. See long --help for more.",
    // long_help = "The key to use for the cache. Can use %{pwd}, %{env} and %{cmd} placeholders.{n}{n}* %{git_uncommitted} contains a hash of the git index and unstaged files.{n}* %{git_head} contains the hash of the git head commit.{n}* %{git} is the combination of all git state.{n}* %{env} only contains non-inherited env."
    // )]
    // pub key: String,
    // // /// Cache based on git state. If the head, index and unstaged changes are the exact same.
    // // ///
    // // /// This is just a short way to set --duration to a long time and --key to '%{git_head}_%{git_uncommitted}.cache'
    // // #[structopt(short = 'g', long = "git", conflicts_with = "duration", conflicts_with = "key")]
    // // pub git: bool,
    // /// Print extra information, e.g. whether the command was run or not.
    // /// When loading from cache, do not show the previous output.
    // #[arg(short = 's', long)]
    // pub no_cached_output: bool,
    // /// Use exit code 0 if the command is cached, and exit code 255 if it ran successfully.
    // #[arg(short = 'e', long)]
    // pub exit_code: bool,

    /// Print extra debug output
    #[arg(short = 'v', long)]
    pub verbose: bool,
    // #[command(subcommand)]
    // pub cmd: CommandArgs,
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
    }
}
