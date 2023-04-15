# bamsalvage, Rust version

Rust version of bamsalvage.

## INTRODUCTION

bamsalvage is a tools to recover sequence reads as much as possible from possibly corrupted BAM files.
This software share the common purpose with bamrescue by Jérémie Roquet (https://bamrescue.arkanosis.net/). 
bamrescue detects corrupted BGZF block using CRC32 checksums and skip corrupted blocks and the method works well if all blocks begin with new reads.

When we would like to recover long-read sequences, a read can span more than one BGZF blocks since the maximum block size is less than sequencer outputs.

Skipping corrupted blocks does not solve such the troubles and often results in termination of Samtools and failure of sequence recovery.

bamsalvage scans next available start positions when any corrupted blocks are detected.
Since the goal of the software is rescuing sequences, bamsalvage do not recover all information included in BAM file but retrieves reads and qual sequences.

## Install
The program requires rustc and cargo (version >= 1.6). All resources will be downloaded and using following commands.
```
git clone https://github.com/takaho/bamsalvage-rust/
cargo build
```

##Usage
`cargo run --release -- -i [BAM file] -o [output file] [--noqual] [--verbose]`
or using binary inside target directory
`bamsalvage -i [BAM file] -o [output file] [--noqual] [--verbose]`

##Commands
```
Options:
  -i, --input <FILE>     Input BAM file
  -o, --output <FILE>    Output filename
  -l, --limit <integer>  Limiting counts [default: 0]
  -n, --noqual           Skip qual field
  -v, --verbose          verbosity
  -h, --help             Print help
  -V, --version          Print version
  ```
