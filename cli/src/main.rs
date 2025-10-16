use std::io;
use std::io::Read;
use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

use tel::tel_build;
use tel::tel_build_str;
use tel::BuildArgs;

#[derive(Parser, Debug)]
#[command(name = "tel")]
struct TelCli {
    #[clap(subcommand)]
    subcommand: SubCmd,
}

#[derive(Parser, Debug)]
#[command(name = "build")]
struct BuildCli {
    /// Path of the file to build
    #[arg(default_value = "./main.tel")]
    pub path: PathBuf,
    /// Print extra debug output
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
#[command(name = "build")]
struct EvalCli {
    /// Text to be evaluated as Tel code
    #[arg()]
    pub code: Option<String>,
    #[arg(short = 'i', long = "stdin", conflicts_with = "code")]
    pub stdin: bool,
    /// Print extra debug output
    #[arg(short = 'v', long)]
    pub verbose: bool,
    #[arg(long)]
    pub debug: bool,
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    Build(BuildCli),
    Script(EvalCli),
}

#[test]
fn test_cli_args() {
    TelCli::try_parse_from(["tel", "build", "-v"]).unwrap();
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "warn"),
    );
    let args = TelCli::parse();
    match args.subcommand {
        SubCmd::Build(build_args) => tel_build(&BuildArgs {
            path: build_args.path,
            verbose: build_args.verbose,
        }),
        SubCmd::Script(script_args) => {
            let code = match (script_args.code, script_args.stdin) {
                (Some(source), false) => source,
                (None, true) => read_source_from_stdin(),
                _ => panic!("must provide either a source string, or --stdin to read input from standard input"),  // TODO @mark: error handling
            };
            tel_build_str(PathBuf::from("script-input"), code, script_args.debug).unwrap(); // TODO @mark: error handling
            eprintln!("built successfully, cannot run yet"); //TODO @mark: impl
            Ok(())
        }
    }
    .unwrap() //TODO @mark: do not unwrap
}

fn read_source_from_stdin() -> String {
    let mut source = String::with_capacity(1024);
    let read = io::stdin()
        .read_to_string(&mut source)
        .expect("could not read stdin"); //TODO @mark: error handling
    assert!(read > 0, "expected stdin to contain input"); //TODO @mark: error handling
    source
}
