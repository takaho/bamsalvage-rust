#![allow(unused)]
#[allow(unused_variables)]

mod bamloader;

use std;
use std::fs;
use std::hash::Hash;
use std::string;
use std::io::Error;
use std::collections::HashMap;
use clap::{Command, Arg, ArgAction};

use std::io::BufReader;
use std::io::prelude::*;
use std::io::{self, BufRead, BufWriter, Write};

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author="Takaho A. Endo")]
#[command(about="Extraction of reads from BAM", long_about="Software extracting seqquence reads as much as possible from possibly corrupted BAM files.")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input BAM file
    #[arg(short, long, value_name="FILE")]
    input: String,

    /// Output filename
    #[arg(short, long, value_name="FILE")]
    output: Option<String>,

    /// Limiting counts
    #[arg(short, long, value_name="integer", default_value="0")]
    limit: usize,

    /// Skip qual field
    #[arg(short, long)]
    noqual:bool,

    /// verbosity
    #[arg(short, long)]
    verbose:bool,
}

fn main() {

    let cli = Cli::parse();

    let input = cli.input;
    let verbose = cli.verbose;
    let limit = cli.limit;
    let noqual = cli.noqual;
    let mut output:Box<dyn Write> = match cli.output {
        Some(v_) => {
            Box::new(BufWriter::new(std::fs::File::create(v_).expect("failed to create a file")))
        },
        None=>Box::new(io::stdout()),
    };

    let info = HashMap::from(
        [
            ("limit", limit as i32), 
            ("verbose", if verbose {1} else {0}),
            ("noqual", if noqual {1} else {0})
        ]
    );

    let mut results:HashMap<String,String> = HashMap::<String,String>::new();
    match bamloader::retrieve_fastq(&input, &mut output, info) {
        Ok(res_)=>{
            for (key,val) in res_ {
                results.insert(key, val);
            }
        },
        Err(e_)=>panic!("{:?}", e_),
    }
    for (key, val) in &results {
        eprintln!("{}={}", key, val);
    }
    println!("{:?}", results);

}