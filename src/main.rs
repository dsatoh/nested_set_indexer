use crate::cli::Format;
use crate::data::Graph;
use cli::Options;
use csv::{ReaderBuilder, WriterBuilder};
use data::Node;
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter};
use structopt::StructOpt;

mod cli;
mod data;
mod error;

fn main() -> error::Result<()> {
    let options = Options::from_args();

    let from = match &options.from {
        Some(v) => v.clone(),
        None => match &options.format_from_input() {
            Some(v) => v.clone(),
            None => Err(error::Error::RuntimeError(format!("missing option --from")))?,
        },
    };
    let to = match &options.to {
        Some(v) => v.clone(),
        None => from.clone(),
    };

    let stdin = io::stdin();
    let input: Box<dyn io::Read> = match &options.input {
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

    let mut graph = Graph::new(data)?;
    if options.complement_leaf {
        if !options.quiet {
            eprintln!("complementing leaf nodes...");
        }
        graph = graph.complement_leaf()?;
    }
    if graph.is_dag() {
        if !options.quiet {
            eprintln!("converting directed acyclic graph to tree...");
        }
        graph = graph.dag_to_tree()?;
    }

    let graph = graph.build_index()?;

    let stdout = io::stdout();
    let output: Box<dyn io::Write> = match &options.output {
        Some(path) => {
            let f = File::create(path)?;
            Box::new(f)
        }
        None => Box::new(stdout.lock()),
    };

    match to {
        Format::JSON => serde_json::to_writer_pretty(BufWriter::new(output), &graph.nodes)?,
        _ => {
            let mut builder = WriterBuilder::new();
            if let Format::TSV = to {
                builder.delimiter(b'\t');
            }

            let mut writer = builder.from_writer(BufWriter::new(output));
            for record in &graph.nodes {
                writer.serialize(record)?;
            }
        }
    };

    Ok(())
}
