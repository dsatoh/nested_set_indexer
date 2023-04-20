use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

#[derive(Debug, Clone, EnumString, EnumVariantNames)]
#[strum(serialize_all = "snake_case")]
pub enum Format {
    CSV,
    TSV,
    JSON,
}

#[derive(Debug, StructOpt)]
pub struct Options {
    /// Complement leaf nodes
    #[structopt(long)]
    pub complement_leaf: bool,

    /// Input format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    pub from: Option<Format>,

    /// Output format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    pub to: Option<Format>,

    /// Output to a file (default: stdout)
    #[structopt(short, long, parse(from_os_str))]
    pub output: Option<PathBuf>,

    /// No output messages
    #[structopt(short, long)]
    pub quiet: bool,

    /// File to process (default: stdin)
    #[structopt(parse(from_os_str))]
    pub input: Option<PathBuf>,
}

impl Options {
    pub fn format_from_input(&self) -> Option<Format> {
        if let Some(input) = self.input.as_ref() {
            if let Some(ext) = input.extension() {
                if let Some(str) = ext.to_str() {
                    return Format::from_str(str).ok();
                }
            }
        }

        None
    }
}
