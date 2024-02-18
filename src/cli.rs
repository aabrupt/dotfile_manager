use clap::{command, Arg, ArgMatches, Command, ValueEnum};

#[derive(Clone, ValueEnum)]
pub enum Direction {
    #[clap(alias = "f")]
    Filesystem,
    #[clap(alias = "d")]
    Dotfiles,
}

pub(crate) fn parse_args() -> ArgMatches {
    command!()
        .subcommands([
            Command::new("sync")
                .arg(
                    Arg::new("direction")
                        .value_parser(
                            clap::builder::EnumValueParser::<Direction>::new(),
                        )
                        .required(true)
                        .help("Provides a the location which should recieve the file update")
                )
                .aliases(["s"])
                .about("Sync files between the file system and source control"),
            Command::new("add")
                .arg(
                    Arg::new("file")
                        .value_parser(clap::builder::PathBufValueParser::new())
                        .required(true)
                        .help("Provides a file to be tracked within the source control"),
                )
                .aliases(["a"])
                .about("Add a file to source control tracking"),
            Command::new("remove")
                .arg(
                    Arg::new("file")
                        .value_parser(clap::builder::PathBufValueParser::new())
                        .required(true)
                        .help("Provides a file to stop being tracked within the source control"),
                )
                .aliases(["r"])
                .about("Remove file from source control tracking"),
        ])
        .subcommand_required(true)
        .get_matches()
}
