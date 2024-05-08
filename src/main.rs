use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
    process::exit,
};

use bytebuffer::ByteBuffer;
use flate2::{write::ZlibEncoder, Compression};

const VERSION: &str = "0.0.1";

fn help() {
    println!("Command usage: kzip [OPTIONS]...");
    println!("Options:");
    println!("  --version      Displays the version");
    println!("  --help         Displays this");
    println!("  --extract -x   Tells kzip to extract a .kzip file");
    println!("  --input   -i   Tells kzip what the input directory or file is");
    println!("  --output  -o   Tells kzip what the output directory or file is");
    println!("  --verbose -v   Shows some possibly useful debug information");
    println!("Information:");
    println!("  KZIP is developed with Rust.");
    println!("  When zipping files, it does not save the directory listing. This means when extracting, it will just plop them out in the output dir.");
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
    let filtered_args: Vec<&String> = args.iter().filter(|f| !f.starts_with("-")).collect();
    let options: Vec<&String> = args.iter().filter(|f| f.starts_with("-")).collect();

    let mut input = "";
    let mut output = "";
    let mut is_extracting = false;
    let mut is_verbose = false;

    if options.len() > 0 {
        for (i, option) in options.iter().enumerate() {
            match option.as_str() {
                "--version" => version(),
                "--help" => help(),
                "--extract" | "-x" => is_extracting = true,
                "--input" | "-i" => input = &filtered_args[i + 1],
                "--output" | "-o" => output = &filtered_args[i + 1],
                "--verbose" | "-v" => is_verbose = true,
                _ => help(),
            }
        }
    } else {
        help()
    }

    if is_verbose {
        println!("kzip: {input} {output}");
    }

    if !is_extracting {
        let output_with_kzip = output.to_owned() + ".kzip";
        if !output.ends_with(".kzip") {
            output = output_with_kzip.as_str();
        }

        let nof = get_number_of_files(&input.to_string());
        let mut file = File::create(output).unwrap();
        let mut buffer = ByteBuffer::new();
        buffer.write_u8(12);
        buffer.write_u8(10);
        buffer.write_u8(116);
        // magic number = cat
        buffer.write_u32(nof); // amount of files

        if let Ok(metadata) = fs::metadata(&input.to_string()) {
            if metadata.is_dir() {
                read_dir(&mut buffer, &input.to_string());
            } else {
                let file_name = Path::new(&input).file_name();
                if let Ok(mut content) =
                    fs::read(format!("{}/{}", "./", file_name.unwrap().to_str().unwrap()))
                {
                    generate_buffer(
                        &mut buffer,
                        file_name.unwrap().to_str().unwrap().to_string(),
                        &mut content,
                    );
                }
            }
        }

        file.write(&buffer.clone().into_vec()).unwrap();
    } else {
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

            let mut nof = buffer.read_u32().unwrap();
            println!("Number of files: {nof}");
            while nof > 0 {
                if let Ok(file_name) = buffer.read_string() {
                    if is_verbose {
                        println!("kzip: File name: {file_name}");
                    }

                    let unpacked_length = buffer.read_u64().unwrap();
                    let length = buffer.read_u64().unwrap();
                    if is_verbose {
                        println!("kzip: File content length: {length}");
                    }

                    let content = decode(
                        &buffer.read_bytes(length.try_into().unwrap()).unwrap(),
                        unpacked_length,
                    );

                    let mut file = File::create(format!("{output}/{file_name}")).unwrap();
                    file.write(&content).unwrap();
                    println!("Unzipped {}", file_name);
                }

                nof -= 1;
            }
        } else {
            println!("kzip: Could not open kzip file {}", input);
            exit(1);
        }
    }

    exit(0);
}

fn generate_buffer(buffer: &mut ByteBuffer, file_name: String, content: &mut Vec<u8>) {
    // buffer.write_u8(file_name.len() as u8);
    buffer.write_string(&file_name);
    buffer.write_u64(content.len() as u64);
    let encoded_content = &mut encode(content);
    buffer.write_u64(encoded_content.len() as u64);
    buffer.write(&encoded_content).unwrap();
}

fn read_dir(mut buffer: &mut ByteBuffer, dir_name: &String) {
    match fs::read_dir(dir_name) {
        Ok(dir_result) => {
            for result in dir_result {
                let entry = result.unwrap();
                let file_name = entry.file_name();
                if let Ok(mut content) =
                    fs::read(format!("{}/{}", dir_name, file_name.to_str().unwrap()))
                {
                    generate_buffer(
                        &mut buffer,
                        file_name.to_str().unwrap().to_string(),
                        &mut content,
                    );
                } else {
                    if let Ok(meta) =
                        fs::metadata(format!("{}/{}", dir_name, file_name.to_str().unwrap()))
                    {
                        if meta.is_dir() {
                            read_dir(
                                buffer,
                                &format!("{}/{}", dir_name, file_name.to_str().unwrap()),
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
