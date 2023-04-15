// #![allow(unused)]
// #[allow(unused_variables)]

use std;
use std::io;
use std::fmt;
use std::str;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write, BufReader, Seek, SeekFrom};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use byteorder::{ByteOrder, LittleEndian};

use flate2::{FlushDecompress, Decompress, Status};
use crc32fast::Hasher;

macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        // Find and cut the rest of the path
        match &name[..name.len() - 3].rfind(':') {
            Some(pos) => &name[pos + 1..name.len() - 3],
            None => &name[..name.len() - 3],
        }
    }};
}

// macro_rules! dbg {
//     ($($x:tt)*) => {
//         {
//             #[cfg(debug_assertions)]
//             {
//                 std::dbg!($($x)*)
//             }
//             #[cfg(not(debug_assertions))]
//             {
//                 ($($x)*)
//             }
//         }
//     }    
// }

#[derive(Debug)]
pub enum BamErrorKind {
    NoBAMFile = 0,
    BlockCorrupted = 1,
    ExceedExpectedSize = 2,
    IncorrectMagicNumber = 3,
    IncorrectGzipMagicNumber = 5,
    BufferTerminated = 4,
    InconsistentChecksum = 6,
    InconsistentBlockSize = 7,
}

#[derive(Debug)]
pub struct BamHandleError {
    line:u32,
    function:String,
    kind:BamErrorKind,
}

impl fmt::Display for BamHandleError {
    fn fmt(&self, ft:&mut fmt::Formatter) -> fmt::Result {
        let msg = match self.kind {
            BamErrorKind::NoBAMFile => "BAM file does not exist.",
            BamErrorKind::BlockCorrupted => "Block corrupted",
            BamErrorKind::ExceedExpectedSize => "Exceed expected block size",
            BamErrorKind::IncorrectMagicNumber => "Incorrect magic number was given",
            BamErrorKind::IncorrectGzipMagicNumber => "Gzip magic number incorrect",
            BamErrorKind::BufferTerminated => "Buffer terminated",
            BamErrorKind::InconsistentChecksum => "Inconsist CRC32 checksum",
            BamErrorKind::InconsistentBlockSize => "Actual size is different size",
                    _ => "Unknown error"
        };
        write!(ft, "{}:{}: {}", self.line, self.function, msg)
    }
}

// Compress text into byte array
// fn compress_text(text:&str)->Result<Vec<u8>,std::io::Error> {
//     let mut buffer = Vec::new();
//     let mut encoder = GzEncoder::new(text.as_bytes(), flate2::Compression::default());
//     encoder.read_to_end(&mut buffer)?;
//     Ok(buffer)
// }

// A function to decompress byte array without gzip header using Decompress
fn decompress_without_header(input: Vec<u8>) -> Result<Vec<u8>, std::io::Error> {
    // Create a new Decompress object with zlib_header set to false
    let mut decompress = Decompress::new(false);//_with_window_bits(false, 15);

    // Create a vector to store the output
    let mut buffer_size = if input.len() < 256 { 1024 } else {input.len() * 4};
    let mut output = Vec::<u8>::with_capacity(buffer_size);

    // Decompress the input using Decompress and write it to the output vector
    let mut status = decompress.decompress_vec(&input, &mut output, FlushDecompress::Finish)?;

    while status != Status::StreamEnd { // if buffer size is less than expected
        buffer_size *= 2;
        if buffer_size > 65535 * 2 { // extracted buffer must be less than 0x100000000
            return Err(std::io::Error::new(ErrorKind::OutOfMemory, "block may be corrupted, size exceeded 65535"));
        }
        let mut extended = Vec::<u8>::with_capacity(buffer_size);
        status = decompress.decompress_vec(
            &input[decompress.total_in() as usize..], 
            &mut extended,
            FlushDecompress::Finish)?;
        output.extend(extended);
    }

    // Return the output vector
    output.shrink_to_fit();
    Ok(output)
}

// fn decopress_with_window_bits(input:&[u8], shift:u8)->Result<Vec<u8>,std::io::Error> {
//     let mut dec = Decompress::new_with_window_bits(false, shift);
//     let mut buf:Vec<u8> = Vec::<u8>::new();
//     dec.decompress(input, &mut buf, flate2::FlushDecompress::Finish)?;
//     Ok(buf)
// }

// Calculate CRC32 checksum
fn calculate_crc32(buffer:&Vec<u8>) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(&buffer);
    hasher.finalize()
}

// fn bytes_to_int(bytes:&[u8]) -> i32 {
//     LittleEndian::read_i32(bytes)
// }

fn read_next_block(reader:&mut BufReader<File>)->Result<Vec<u8>, BamHandleError> {
    // Read first 4 bytes of ID1, ID2, CM, FLG
    let mut buf:[u8;4] = [0;4];
    match reader.read_exact(&mut buf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }
    // println!("First 4 bytes : {}, {}, {}, {}", buf[0], buf[1], buf[2], buf[3]);
    // Scan until identifier and constant match
    loop {
        if buf == [31, 139, 8, 4] {
            break;
        }
        let mut onebyte:[u8;1] = [0;1];
        match reader.read_exact(&mut onebyte) {
            Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
            _ => (),
        }
        buf.rotate_left(1);
        buf[3] = onebyte[0];
        // println!("First 4 bytes : {}, {}, {}, {}", buf[0], buf[1], buf[2], buf[3]);
    }
    // read MTIME(u16), XFL(u8), OS(u8), XLEN(u16)
    let mut buf:[u8;8] = [0;8];
    match reader.read_exact(&mut buf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }
    let xlen = LittleEndian::read_u16(&buf[6..8]);
    // println!("extra subfield : {}", xlen);

    // SI1(u8), SI2(u8), SLEN(u16), BSIZE(u16)
    let mut xbuf = Vec::<u8>::with_capacity(xlen as usize);
    unsafe {
        xbuf.set_len(xlen as usize);
    }
    // let mut xbuf:[MaybeUninit<Vec<u8>>;xlen as usize] = unsafe {
    //     MaybeUninit::uninit().assume_init()
    // };
    // let mut xbuf = vec![0u8;xlen as usize];
    match reader.read_exact(&mut xbuf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }

    if xbuf[0..2] != [66, 67] {
        return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BlockCorrupted});
    }
    let subfield_length = LittleEndian::read_u16(&xbuf[2..4]);
    let block_size = LittleEndian::read_u16(&xbuf[4..6]);
    // println!("xlen = {}, subfield length = {}, block size = {}", xlen, subfield_length, block_size);
    let compressed_data_size = block_size - xlen - 19;
    // println!("compressed data size {} ", compressed_data_size);

    let mut cdata = vec![0u8;compressed_data_size as usize];
    match reader.read_exact(&mut cdata) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }
    // read CRC32 and expected size
    let mut tbuf:[u8;8] = [0;8];
    match reader.read_exact(&mut tbuf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }

    let mut buffer:Vec<u8> = Vec::<u8>::new();
    let mut crc32_calc:u32 = 0;
    let mut crc32_file:u32 = 0;
    let input_size = cdata.len();

    match decompress_without_header(cdata) {
        Ok(b_)=>buffer=b_, // ; text = String::from_utf8(b_).unwrap()},
        // Ok(b_)=>{println!("OK! : {} bytes", b_.len()); buffer=b_}, // ; text = String::from_utf8(b_).unwrap()},
        Err(e_) => {
            println!("{:?}", e_);
            return Err(
                BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BlockCorrupted}
                )
            },
    }
    // cdata
    crc32_calc = calculate_crc32(&buffer);
    crc32_file = LittleEndian::read_u32(&tbuf[0..4]);
    if crc32_calc != crc32_file {
        println!("CRC32 : {:X} <=> {:X} ({} -> {} bytes)", crc32_calc, crc32_file, input_size, buffer.len());
        return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::InconsistentChecksum});
    }

    let input_size = LittleEndian::read_u32(&tbuf[4..8]) as usize;
    if input_size != buffer.len() {
        return Err(
            BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::InconsistentBlockSize}
            )
    }
    // println!("{}:{} {} bytes, CRC32={:x}", function!().to_string(), line!(), buffer.len(), crc32_file);

    Ok(buffer)
}

fn scan_next_block(reader:&mut BufReader<File>)->Result<Vec<u8>, BamHandleError> {
    // ID1   0-0 u8 = 31 
    // ID2   1-1 u8 = 139
    // CM    2-2 u8 = 8
    // FLG   3-3 u8 = 4
    // MTIME 4-7 u32
    // XFL   8-8 u8
    // OS    9-9 u8
    // XLEN  10-11 u16
    // SI1    | u8
    // SI2    | u8
    // SLEN   | u16
    // BSIZE  | u16 12-12 + XLEN (min 6)
    // CDATA u8[BSIZE-XLEN-19]
    // CRC32 u32
    // ISIZE u32

    let mut buf:[u8;18] = [0;18];
    // let mut onebyte:[u8;1] = [0;1];
    let mut xlen:usize = 0;
    let mut block_size:usize = 0;
    match reader.read_exact(&mut buf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }
    loop { // scan
        if buf[0..4] == [31,139,8,4] && buf[12..14] == [66,67] {
            xlen = LittleEndian::read_u16(&buf[10..12]) as usize;
            block_size = LittleEndian::read_u16(&buf[16..18]) as usize;
            // read extra xlen - 2 bytes
            if xlen >= 6 && block_size > xlen + 19 {
                let mut nullbuf:Vec<u8> = Vec::<u8>::with_capacity(xlen - 6);
                match reader.read_exact(&mut nullbuf) {
                    Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
                    _ => (),
                }
                break;
            }
        }

        let mut shift_bytes:usize = 18;
        for i in 1..18 {
            if buf[i] == 31 {
                shift_bytes = i;
                for j in 0..i {
                    buf[j] = buf[i+j];
                }
            }
        }
        if shift_bytes > 0 {
            for i in 0..shift_bytes {
                buf[i] = buf[i + shift_bytes];
            }
        }
        match reader.read_exact(&mut buf[18-shift_bytes..18]) {
            Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
            _ => (),
        }
        // buf.rotate_left(1);
        // match reader.read_exact(&mut onebyte) {
        //     Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        //     _ => (),
        // }
        // buf[buf.len() - 1] = onebyte[0];
    }

    // let subfield_length = LittleEndian::read_u16(&xbuf[2..4]);
    // let block_size = LittleEndian::read_u16(&xbuf[4..6]);
    // println!("xlen = {}, subfield length = {}, block size = {}", xlen, subfield_length, block_size);
    let compressed_data_size = block_size - xlen - 19;
    let mut cdata = vec![0u8;compressed_data_size as usize];
    match reader.read_exact(&mut cdata) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }
    // read CRC32 and expected size
    let mut tbuf:[u8;8] = [0;8];
    match reader.read_exact(&mut tbuf) {
        Err(_err)=>return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BufferTerminated}),
        _ => (),
    }

    let mut buffer:Vec<u8> = Vec::<u8>::new();
    let mut crc32_calc:u32 = 0;
    let mut crc32_file:u32 = 0;
    let input_size = cdata.len();

    match decompress_without_header(cdata) {
        Ok(b_)=>buffer=b_, // ; text = String::from_utf8(b_).unwrap()},
        // Ok(b_)=>{println!("OK! : {} bytes", b_.len()); buffer=b_}, // ; text = String::from_utf8(b_).unwrap()},
        Err(e_) => {
            eprintln!("{:?}", e_);
            return Err(
                BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::BlockCorrupted}
                )
            },
    }
    // cdata
    crc32_calc = calculate_crc32(&buffer);
    crc32_file = LittleEndian::read_u32(&tbuf[0..4]);
    if crc32_calc != crc32_file {
        eprintln!("CRC32 : {:X} <=> {:X} ({} -> {} bytes)", crc32_calc, crc32_file, input_size, buffer.len());
        return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::InconsistentChecksum});
    }

    let input_size = LittleEndian::read_u32(&tbuf[4..8]) as usize;
    if input_size != buffer.len() {
        return Err(
            BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::InconsistentBlockSize}
            )
    }
    // println!("{}:{} {} bytes, CRC32={:x}", function!().to_string(), line!(), buffer.len(), crc32_file);

    Ok(buffer)
}

// Convert 2-nuc encoded byte array into sequence
fn convert_sequence(buffer:&Vec<u8>, start:usize, length:usize) -> String {
    static BASES:[u8;16] = [61, 65, 67, 77, 71, 82, 83, 86, 84, 87, 89, 72, 75, 68, 66, 78];
    let mut seq:Vec<u8> = Vec::<u8>::with_capacity(length + 1);
    unsafe {
        seq.set_len(length + 1);
    }
    let span = ((length + 1) / 2);

    for i in 0..span {
        if i + start >= buffer.len() {
            panic!("out of range in seq i={}/span={}/len={} buffer size={}", i, span, length, buffer.len());
        }
        let b = buffer[i+start];
        let b1 = b & 0x0f;
        let b0 = b >> 4;
        // println!("{} {};{} {};{}", i, b0, b1, BASES[b0 as usize] as char, BASES[b1 as usize] as char);
        seq[i * 2] = BASES[b0 as usize];
        seq[i * 2 + 1] = BASES[b1 as usize];
    }
    unsafe {
        seq.set_len(length);
    }
    String::from_utf8(seq).unwrap()
}

// Convert QUAL values into string
fn convert_qual(buffer:&Vec<u8>, start:usize, length:usize) -> String {
    let mut seq:Vec<u8> = Vec::<u8>::with_capacity(length);
    unsafe {
        seq.set_len(length);
    }

    for i in 0..length {
        let b:u8 = buffer[i + start];
        if b >= 135 { // invalid value
            return "?".to_string();
        }
        seq[i] = 33 + buffer[i+start];
    }
    String::from_utf8(seq).unwrap_or("?".to_string())
}

// dumping hex
fn get_hex_string(buffer:&Vec<u8>, pos:usize, span:usize) -> String {
    let mut hexstr:String = String::new();
    for i in 0..span {
        hexstr = [hexstr, format!("{:02X}", buffer[pos + i - span / 2])].join(" ");
    }    
    return hexstr;
}

pub fn retrieve_fastq(filename_bam:&String, output:&mut Box<dyn Write>, info:HashMap<&str,i32>)
    ->Result<HashMap<String,String>, BamHandleError> {
    let mut results:HashMap<String,String> = HashMap::new();
    let mut n_seqs:i64 = 0;
    let mut n_blocks:i64 = 0;
    let mut n_corrupted_blocks:i64 = 0;
    let mut verbose:bool = false;
    let mut noqual:bool = false;
    let mut limit:i64 = 0;

    // println!("{:?}", info.get("verbose"));
    eprintln!("input={}", filename_bam);
    for (key, val) in info {
        eprintln!("{} = {}", key, val);
        if key == "verbose" && val > 0 {
            verbose = true;
        } else if key == "noqual" && val > 0 {
            noqual = true;
        } else if key == "limit" {
            limit = val as i64;
        }
    }

    // output.write("hello".as_bytes());

    // read header
    // fn read_next_block(handler:&mut BufReader)->Result<Vec<u8>, Box<error::Error>> {
    let file_in = std::fs::File::open(filename_bam).unwrap();
    let mut filesize = 0;
    match file_in.metadata() {
        Ok(m_) => filesize = m_.len(),
        Err(e_) => {
            return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::NoBAMFile});
        },
    }

    let mut reader:BufReader<File> = BufReader::new(file_in);
    let mut buffer:Vec<u8> = Vec::new();

    // header, if the data block is corrupted, skip the part 
    buffer = scan_next_block(&mut reader)?;
    // Assert BAM\1
    if buffer[0..4] != [66, 65, 77, 1] {
        return Err(BamHandleError{line:line!(), function:function!().to_string(), kind:BamErrorKind::IncorrectMagicNumber});
    }
    let l_text = LittleEndian::read_u32(&buffer[4..8]) as usize;
    let l_buffer = buffer.len() as usize;
    let mut scanmode:bool = false;
    buffer.clear();

    // reader.seek(SeekFrom::Start(13520742501));//SeekFrom::Start(filesize * 18 / 100));

    loop {
        // read a block (from 0)
        if scanmode {
            match scan_next_block(&mut reader) {
                Ok(mut _data) => {
                    scanmode = false;
                    buffer = _data;
                },
                Err(_err) => { 
                    #[cfg(debug_assertions)]
                    eprintln!("no BGZF block found : {}", _err); 
                    break;
                },
            }
        } else {
            match read_next_block(&mut reader) {
                Ok(mut _data) => {if buffer.len() == 0 {buffer=_data} else {buffer.append(&mut _data)}},
                Err(_err) => {
                    #[cfg(debug_assertions)]
                    {
                        let current_pos = reader.seek(SeekFrom::Current(0)).unwrap();
                        eprintln!("corrupted block detected at {}.", current_pos);
                    }
                    // eprintln!("{}", _err); 
                    n_corrupted_blocks += 1;
                    scanmode = true;
                    continue;
                },
            }
        }
        n_blocks += 1;

        // read fields 
        while buffer.len() >= 36 {
            // block_size u32  0-3
            // refID i32       4-7
            // pos i32         8-11
            // l_read_name u8  12
            // mapq u8         13
            // bin u16         14-15
            // n_cigar_op u16  16-17
            // flag u16        18-19
            // l_seq u32       20-23
            // next_refID i32  24-27
            // next_pos i32    28-31
            // tlen i32        32-35
            // read_name u8 * l_read_name +36
            // cigar u32 * n_cigar_op
            // seq u8 * (l_seq+1)/2
            // qual char[l_seq]
            let block_size = LittleEndian::read_u32(&buffer[0..4]) as usize;
            let l_read_name = buffer[12] as usize;
            let l_seq = LittleEndian::read_u32(&buffer[20..24]) as usize;
            let tlen = LittleEndian::read_i32(&buffer[32..36]);
            let ptr_block_start = 4;
            let drain_pos = ptr_block_start + block_size;
            let n_cigar_op = LittleEndian::read_u16(&buffer[16..18]) as usize;

            let seq_ptr = l_read_name + 36 + n_cigar_op * 4;
            let minimum_buffer_size = seq_ptr + l_seq * (if noqual {1} else {3}) / 2;
            if drain_pos <= 36 || drain_pos < minimum_buffer_size { // bad drain position
                n_corrupted_blocks += 1;
                scanmode = true;
            } else {
                // fill buffer
                while drain_pos > buffer.len() || buffer.len() < minimum_buffer_size {
                    let prevsize = buffer.len();
                    match read_next_block(&mut reader) {
                        Ok(mut _data)=>{
                            buffer.append(&mut _data);
                            n_blocks += 1;
                        },
                        Err(e_)=>{
                            let current_pos = reader.seek(SeekFrom::Current(0)).unwrap();
                            #[cfg(debug_assertions)]
                            eprintln!("{}:corrupted block detected at {}   ", line!(), current_pos);
                            scanmode = true;
                            break;
                        },
                    }
                }
            }
            if scanmode || drain_pos < 36 || buffer.len() < 36 || l_read_name < 3 {
                buffer.clear();
                n_corrupted_blocks += 1;
                break;
            }
            // std::process::exit(0);
            // process sequence (and qual) even if block corrupted
            // eprintln!("drain_pos={}, buffer_size={}, minimum={}\n", drain_pos, buffer.len(), minimum_buffer_size);

            // eprintln!("{} at {}", function!().to_string(), line!());
            if drain_pos <= buffer.len() { // sequence contained in the block
                // eprintln!("\n{} {}\n", buffer.len(), l_read_name);
                let seq_name = str::from_utf8(&buffer[36..35+l_read_name]).unwrap();

                if seq_ptr + minimum_buffer_size > buffer.len() { // overflow
                    #[cfg(debug_assertions)]
                    eprintln!("sequence position overflow the block minimal_size={}/buffer_size={}",
                        minimum_buffer_size, buffer.len());
                    n_corrupted_blocks += 1;
                    scanmode = true;
                    break;
                }
                // println!("{}\t{}\t{}", seq_name, l_seq, n_cigar_op);
                // skip CIGAR and read SEQ and QUAL
                let sequence = convert_sequence(&buffer, seq_ptr, l_seq);
                let mut seq_display:String;
                if l_seq != sequence.len() {
                    #[cfg(debug_assertions)]
                    eprintln!("{} had bad SEQ", seq_name);
                    n_corrupted_blocks += 1;
                    scanmode = true;
                    break;
                } else if l_seq > 40 {
                    seq_display = format!("{}..{}", &sequence[0..20], &sequence[l_seq-20..]);
                } else if l_seq > 20 {
                    seq_display = format!("{}..", &sequence[0..20]);
                } else {
                    seq_display = sequence.clone();
                }

                if noqual {
                    output.write(format!(">{}\n{}\n", seq_name, sequence).as_bytes());
                    // println!("{}\t{}\t{}", seq_name, l_seq, seq_display);
                } else {
                    let qual = convert_qual(&buffer, seq_ptr + (l_seq + 1)/ 2, l_seq);
                    let mut qual_display:String;
                    if qual.len() != l_seq { // invalid character in qual string
                        #[cfg(debug_assertions)]
                        eprintln!("{} had bad QUAL", seq_name);
                        n_corrupted_blocks += 1;
                        scanmode = true;
                        break;
                    } else if l_seq > 40 {
                        qual_display = format!("{}..{}", &qual[0..20], &qual[l_seq-20..]);
                    } else if l_seq > 20 {
                        qual_display = format!("{}..", &qual[0..20]);
                    } else {
                        qual_display = qual.clone();
                    }
                    output.write(format!("@{}\n{}\n+\n{}\n", seq_name, sequence, qual).as_bytes());
                    // println!("{}\t{}\t{}\t{}", seq_name, l_seq, seq_display, qual_display);
                }
                n_seqs += 1;

                if verbose && n_seqs % 1000 == 0 {
                    let current_pos = reader.seek(SeekFrom::Current(0)).unwrap();
                    eprint!("\x1B {:.1}% {}k reads / {}k blocks / {} corrupted  {}\r", 
                        current_pos as f32 * 100.0 / (filesize as f32),
                        n_seqs / 1000, n_blocks / 1000, n_corrupted_blocks, seq_name)
                }
                buffer.drain(0..drain_pos);
            } // process a read
        }
        if limit > 0 && n_seqs >= limit {
            break;
        }
    }

    results.insert("n_sequences".to_string(), format!("{}", n_seqs).to_string());
    results.insert("n_blocks".to_string(), format!("{}", n_blocks).to_string());
    results.insert("n_corrupted".to_string(), format!("{}", n_corrupted_blocks).to_string());
    Ok(results)
}