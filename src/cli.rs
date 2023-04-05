use crate::{error, NestedSet, Node};
use csv::{ReaderBuilder, WriterBuilder};
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

#[derive(Debug, Clone, EnumString, EnumVariantNames)]
#[strum(serialize_all = "snake_case")]
enum Format {
    CSV,
    TSV,
    JSON,
}

#[derive(Debug, StructOpt)]
pub struct Options {
    /// Input format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    from: Option<Format>,

    /// Output format
    #[structopt(short, long, possible_values = Format::VARIANTS)]
    to: Option<Format>,

    /// Output to a file (default: stdout)
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,

    /// File to process (default: stdin)
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

impl Options {
    pub fn run(&self) -> error::Result<()> {
        let from = match self.from.as_ref() {
            Some(v) => v.clone(),
            None => match self.format_from_input().as_ref() {
                Some(v) => v.clone(),
                None => Err(error::Error::RuntimeError(format!("missing option --from")))?,
            },
        };
        let to = match self.to.as_ref() {
            Some(v) => v.clone(),
            None => from.clone(),
        };

        let stdin = io::stdin();
        let input: Box<dyn io::Read> = match self.input.as_ref() {
            Some(path) => {
                let f = File::open(path)?;
                Box::new(f)
            }
            None => Box::new(stdin.lock()),
        };

        let data = match from {
            Format::JSON => serde_json::from_reader(BufReader::new(input))?,
            _ => {
                let mut builder = ReaderBuilder::new();
                if let Format::TSV = from {
                    builder.delimiter(b'\t');
                }

                let mut reader = builder.from_reader(BufReader::new(input));
                reader
                    .deserialize()
                    .filter_map(|x| x.ok())
                    .collect::<Vec<Node>>()
            }
        };

        let mut set = NestedSet::new(data)?;
        let set = set.rebuild()?;

        let stdout = io::stdout();
        let output: Box<dyn io::Write> = match self.output.as_ref() {
            Some(path) => {
                let f = File::create(path)?;
                Box::new(f)
            }
            None => Box::new(stdout.lock()),
        };

        match to {
            Format::JSON => serde_json::to_writer_pretty(BufWriter::new(output), &set.nodes)?,
            _ => {
                let mut builder = WriterBuilder::new();
                if let Format::TSV = to {
                    builder.delimiter(b'\t');
                }

                let mut writer = builder.from_writer(BufWriter::new(output));
                for record in &set.nodes {
                    writer.serialize(record)?;
                }
            }
        };

        Ok(())
    }

    fn format_from_input(&self) -> Option<Format> {
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
