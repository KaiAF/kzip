use std::{
    cmp,
    collections::HashMap,
    env,
    fs::{self, File, Metadata},
    io::{ErrorKind, Write},
    path::{self, Path},
    process::exit,
    time::UNIX_EPOCH,
};

use bytebuffer::ByteBuffer;
use flate2::{write::ZlibEncoder, Compression};
use sha256::digest;
use time::OffsetDateTime;

const VERSION: &str = "0.0.7";

fn help() {
    println!("Command usage: kzip [OPTIONS]...");
    println!("Options:");
    println!("  --version      Displays the version");
    println!("  --help         Displays this");
    println!("  --extract -x   Tells kzip to extract a .kzip file");
    println!("  --ls      -l   Displays zipped files inside a .kzip file");
    println!("  --input   -i   Tells kzip what the input directory or file is");
    println!("  --output  -o   Tells kzip what the output directory or file is");
    println!("  --verbose -v   Shows some possibly useful debug information");
    println!("Information:");
    println!("  KZIP is developed with Rust.");
    println!("  When zipping files, KZIP uses GZIP's best compression.");
    println!("Contact me at https://github.com/KaiAF/kzip/issues");

    exit(0);
}

fn version() {
    println!("kzip: version {VERSION}");
    exit(0);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let options: Vec<&String> = args.iter().filter(|f| f.starts_with("-")).collect();

    let mut input = "";
    let mut output = "";
    let mut is_extracting = false;
    let mut is_verbose = false;
    let mut show_files = false;

    if options.len() > 0 {
        for (_i, option) in options.iter().enumerate() {
            match option.as_str() {
                "--version" => version(),
                "--help" => help(),
                "--extract" | "-x" => is_extracting = true,
                "--input" | "-i" => {
                    input = {
                        let index = args
                            .iter()
                            .position(|f| f.eq_ignore_ascii_case(option.as_str()))
                            .unwrap();
                        &args[index + 1]
                    }
                }
                "--output" | "-o" => {
                    output = {
                        let index = args
                            .iter()
                            .position(|f| f.eq_ignore_ascii_case(option.as_str()))
                            .unwrap();
                        &args[index + 1]
                    }
                }
                "--verbose" | "-v" => is_verbose = true,
                "--ls" | "-l" => show_files = true,
                _ => help(),
            }
        }
    } else {
        help()
    }

    if output.is_empty() {
        output = input;
    }

    if is_verbose {
        println!("kzip:\ninput: {input}\noutput: {output}");
    }

    if show_files {
        if let Ok(file) = fs::read(input) {
            let mut buffer = ByteBuffer::from_bytes(&file);
            let mut mk = 0;

            mk += buffer.read_u8().unwrap();
            mk += buffer.read_u8().unwrap();
            mk += buffer.read_u8().unwrap();

            if mk != 138 {
                println!("kzip: {}: Invalid KZip header", input);
                exit(1);
            }

            let version = buffer.read_string().unwrap();
            let mut nof = buffer.read_u32().unwrap();
            let og_nof = nof.clone();
            let mut total_length = 0;
            let mut total_unpacked_length = 0;

            while nof > 0 {
                let is_duplicate: u8 = buffer.read_u8().unwrap();
                if let Ok(file_name) = buffer.read_string() {
                    let created_at = buffer.read_u64().unwrap();
                    let modified = buffer.read_u64().unwrap();

                    if is_duplicate == 1 {
                        let _bytes = buffer.read_u32().unwrap();
                        println!("{file_name} (duplicate)");
                    } else {
                        let unpacked_length = buffer.read_u64().unwrap();
                        let length = buffer.read_u64().unwrap();
                        let _bytes = buffer.read_bytes(length.try_into().unwrap()).unwrap();

                        total_length += length;
                        total_unpacked_length += unpacked_length;

                        if is_verbose {
                            println!(
                                "{file_name}\n  Created At: {}, Last Modified: {}\n  packed: {}, unpacked: {}",
                                OffsetDateTime::from_unix_timestamp(created_at as i64).unwrap().date(),
                                OffsetDateTime::from_unix_timestamp(modified as i64).unwrap().date(),
                                format_byte(length as f64),
                                format_byte(unpacked_length as f64)
                            );
                        } else {
                            println!("{file_name}");
                        }
                    }
                }

                nof -= 1;
            }

            if is_verbose {
                println!("Version used to zip: {version}");
            }

            println!("Total Files: {og_nof}");
            println!("Total Packed Size: {}", format_byte(total_length as f64));
            println!(
                "Total Unpacked Size: {}",
                format_byte(total_unpacked_length as f64)
            );
            println!("Compression: {}%", total_unpacked_length / total_length);

            exit(0);
        } else {
            println!("kzip: could not read {input}");
            exit(1);
        }
    }

    if !is_extracting {
        let output_with_kzip = output.to_owned() + ".kzip";
        if !output.ends_with(".kzip") {
            output = output_with_kzip.as_str();
        }

        let nof = get_number_of_files(&input.to_string());
        let mut hashes: HashMap<String, usize> = HashMap::new();
        let mut file = File::create(output).unwrap();
        let mut buffer = ByteBuffer::new();
        buffer.write_u8(12);
        buffer.write_u8(10);
        buffer.write_u8(116);
        // magic number = cat
        buffer.write_string(VERSION); // version
        buffer.write_u32(nof); // amount of files

        if let Ok(metadata) = fs::metadata(&input.to_string()) {
            if metadata.is_dir() {
                read_dir(&mut buffer, &input.to_string(), is_verbose, &mut hashes);
            } else {
                let file_name = Path::new(&input).file_name();
                if let Ok(mut content) = fs::read(file_name.unwrap().to_str().unwrap()) {
                    generate_buffer(
                        &mut buffer,
                        file_name.unwrap().to_str().unwrap().to_string(),
                        &mut content,
                        &metadata,
                        &mut hashes,
                    );
                }
            }
        }

        file.write(&buffer.clone().into_vec()).unwrap();
        println!("kzip: Done zipping");
    } else {
        if let Ok(file) = fs::read(input) {
            let mut index = 0;
            let mut cached: HashMap<u32, String> = HashMap::new();
            let mut buffer = ByteBuffer::from_bytes(&file);
            let mut mn = 0;

            mn += buffer.read_u8().unwrap();
            mn += buffer.read_u8().unwrap();
            mn += buffer.read_u8().unwrap();

            if mn != 138 {
                println!("kzip: {}: Invalid KZip header", input);
                exit(1);
            }

            create_dir_if_not_exists(&output);

            let _version = buffer.read_string().unwrap();
            let mut nof = buffer.read_u32().unwrap();
            println!("Number of files: {nof}");
            while nof > 0 {
                let is_duplicate: u8 = buffer.read_u8().unwrap();
                if let Ok(file_name) = buffer.read_string() {
                    if is_verbose {
                        println!("kzip: File name: {file_name}");
                    }

                    // todo: modify the file and put the correct created at and modified dates;
                    let _created_at = buffer.read_u64().unwrap();
                    let _modified = buffer.read_u64().unwrap();

                    if is_duplicate == 1 {
                        // handle duplication
                        let saved_index = buffer.read_u32().unwrap();
                        let cached_file_name = cached.get(&saved_index).unwrap();
                        let content = fs::read(cached_file_name).unwrap();

                        let formatted_output =
                            format!("{output}{}{file_name}", path::MAIN_SEPARATOR);
                        let split_paths: Vec<&str> =
                            formatted_output.split(path::MAIN_SEPARATOR).collect();
                        let dir_name =
                            &split_paths[0..split_paths.len() - 1].join(path::MAIN_SEPARATOR_STR);

                        create_dir_if_not_exists(&dir_name);

                        let mut file =
                            File::create(format!("{output}{}{file_name}", path::MAIN_SEPARATOR))
                                .unwrap();

                        file.write(&content).unwrap();
                        println!("Unzipped duplicate file {}", file_name);
                    } else {
                        let unpacked_length = buffer.read_u64().unwrap();
                        let length = buffer.read_u64().unwrap();
                        if is_verbose {
                            println!("kzip: File content length: {length}");
                        }

                        let content = decode(
                            &buffer.read_bytes(length.try_into().unwrap()).unwrap(),
                            unpacked_length,
                        );

                        let formatted_output =
                            format!("{output}{}{file_name}", path::MAIN_SEPARATOR);
                        let split_paths: Vec<&str> =
                            formatted_output.split(path::MAIN_SEPARATOR).collect();
                        let dir_name =
                            &split_paths[0..split_paths.len() - 1].join(path::MAIN_SEPARATOR_STR);

                        create_dir_if_not_exists(&dir_name);

                        let mut file =
                            File::create(format!("{output}{}{file_name}", path::MAIN_SEPARATOR))
                                .unwrap();

                        file.write(&content).unwrap();

                        cached.insert(
                            index,
                            format!("{output}{}{file_name}", path::MAIN_SEPARATOR),
                        );

                        index += 1;

                        println!("Unzipped {}", file_name);
                    }
                }

                nof -= 1;
            }

            println!("kzip: Done unzipping");
        } else {
            println!("kzip: Could not open kzip file {}", input);
            exit(1);
        }
    }

    exit(0);
}

fn generate_buffer(
    buffer: &mut ByteBuffer,
    file_name: String,
    content: &mut Vec<u8>,
    metadata: &Metadata,
    hashes: &mut HashMap<String, usize>,
) {
    let modified = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let created_at = metadata
        .created()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let file_hash = digest(content.clone());
    let is_duplicate = if !metadata.is_dir() {
        hashes.contains_key(&file_hash)
    } else {
        false
    };

    buffer.write_u8(if is_duplicate { 1 } else { 0 }); // tell kzip if file is duplicate

    // buffer.write_u8(file_name.len() as u8);
    buffer.write_string(&file_name);
    buffer.write_u64(created_at);
    buffer.write_u64(modified);
    if is_duplicate {
        // there is a duplicate file found
        // going to tell kzip this to save some space
        buffer.write_u32((*hashes.get(&file_hash).unwrap()).try_into().unwrap());
    } else {
        buffer.write_u64(content.len() as u64);
        let encoded_content = &mut encode(content);
        buffer.write_u64(encoded_content.len() as u64);
        buffer.write(&encoded_content).unwrap();
        if !metadata.is_dir() {
            hashes.insert(file_hash, hashes.len());
        }
    }
}

fn read_dir(
    mut buffer: &mut ByteBuffer,
    dir_name: &String,
    verbose: bool,
    hashes: &mut HashMap<String, usize>,
) {
    match fs::read_dir(dir_name) {
        Ok(dir_result) => {
            for result in dir_result {
                let entry = result.unwrap();
                let file_name = entry.file_name();
                if let Ok(mut content) = fs::read(format!(
                    "{}{}{}",
                    dir_name,
                    path::MAIN_SEPARATOR,
                    file_name.to_str().unwrap()
                )) {
                    if verbose {
                        println!("kzip: reading file: {}", file_name.to_str().unwrap());
                    }

                    generate_buffer(
                        &mut buffer,
                        format!(
                            "{}{}{}",
                            dir_name,
                            path::MAIN_SEPARATOR,
                            file_name.to_str().unwrap()
                        ),
                        &mut content,
                        &entry.metadata().unwrap(),
                        hashes,
                    );
                } else {
                    if let Ok(meta) = fs::metadata(format!(
                        "{}{}{}",
                        dir_name,
                        path::MAIN_SEPARATOR,
                        file_name.to_str().unwrap()
                    )) {
                        if meta.is_dir() {
                            if verbose {
                                println!("kzip: reading directory: {}", dir_name);
                            }

                            read_dir(
                                buffer,
                                &format!(
                                    "{}{}{}",
                                    dir_name,
                                    path::MAIN_SEPARATOR,
                                    file_name.to_str().unwrap(),
                                ),
                                verbose,
                                hashes,
                            );
                        }
                    } else {
                        println!("kzip: could not read file {}", file_name.to_str().unwrap());
                    }
                }
            }
        }
        Err(err) => {
            println!("kzip: there was an error");
            println!("{:#?}", err)
        }
    }
}

fn get_number_of_files(dir_name: &String) -> u32 {
    let mut i = 0;

    match fs::read_dir(dir_name) {
        Ok(dir) => {
            for entry in dir {
                if let Ok(metadata) = fs::metadata(dir_name) {
                    if metadata.is_dir() {
                        i += get_number_of_files(&format!(
                            "{dir_name}/{}",
                            entry.unwrap().file_name().into_string().unwrap()
                        ));
                    } else {
                        i += 1;
                    }
                }
            }
        }
        Err(_err) => {
            if let Ok(metadata) = fs::metadata(dir_name) {
                if metadata.is_file() {
                    i += 1;
                }
            }
        }
    }

    return i;
}

fn encode(bytes: &mut [u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    let _ = e.write_all(bytes);
    let compressed_bytes = e.finish().unwrap();

    return compressed_bytes.to_vec();
}

fn decode(bytes: &[u8], file_size: u64) -> Vec<u8> {
    let input = bytes;
    let mut decompressor = flate2::Decompress::new(true);
    let mut buf = Vec::with_capacity(file_size.try_into().unwrap());
    decompressor
        .decompress_vec(&input, &mut buf, flate2::FlushDecompress::None)
        .unwrap();

    return buf;
}

/*
    Stolen from: https://github.com/banyan/rust-pretty-bytes/blob/master/src/converter.rs
*/
fn format_byte(num: f64) -> String {
    let negative = if num.is_sign_positive() { "" } else { "-" };
    let num = num.abs();
    let units = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    if num < 1_f64 {
        return format!("{}{} {}", negative, num, "B");
    }
    let delimiter = 1000_f64;
    let exponent = cmp::min(
        (num.ln() / delimiter.ln()).floor() as i32,
        (units.len() - 1) as i32,
    );
    let pretty_bytes = format!("{:.2}", num / delimiter.powi(exponent))
        .parse::<f64>()
        .unwrap()
        * 1_f64;
    let unit = units[exponent as usize];
    format!("{}{} {}", negative, pretty_bytes, unit)
}

fn create_dir_if_not_exists(output: &str) {
    if let Err(err) = fs::metadata(output) {
        if err.kind() == ErrorKind::NotFound {
            // directory does not exist, so create it
            if let Err(err_dir) = fs::create_dir_all(output) {
                println!("kzip: There was an error creating directory {output}");
                println!("{:#?}", err_dir);
                exit(1);
            }
        } else {
            println!("kzip: There was an error writing to {output}");
            println!("{:#?}", err);
            exit(1);
        }
    }
}
