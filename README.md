# bamsalvage, Rust version
Rust version of bamsalvage

Native command of bamsalvage, extracting software of sequence reads from possibly corrupted BAM files.

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
