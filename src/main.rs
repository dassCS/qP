use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use brotli::enc::BrotliEncoderParams;
use brotli::{BrotliCompress, BrotliDecompress};
use clap::{Parser, Subcommand};
use image::{DynamicImage, ImageBuffer, ImageFormat};

/// QP Image Tool
#[derive(Parser)]
#[command(name = "qp")]
#[command(about = "Encode and decode QP image files (.qp)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode an image to QP format
    Encode {
        /// Input image file path
        input: PathBuf,
        /// Output QP file path
        output: PathBuf,
    },
    /// Decode a QP file to an image
    Decode {
        /// Input QP file path
        input: PathBuf,
        /// Output image file path
        output: PathBuf,
    },
}

const MAGIC: &[u8; 4] = b"QPIM";
const COMPRESSION_METHOD_BROTLI: u8 = 1;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode { input, output } => {
            if let Err(e) = encode_image(&input, &output) {
                eprintln!("Error encoding image: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Decode { input, output } => {
            if let Err(e) = decode_image(&input, &output) {
                eprintln!("Error decoding QP image: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Encode a standard image to QP format
fn encode_image(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Load the image
    let img = image::open(input_path)?.to_rgba8();
    let (width, height) = img.dimensions();
    let channels = 4; // RGBA

    // Get raw pixel data
    let pixel_data = img.into_raw();

    // Configure Brotli encoder parameters for better compression
    let mut params = BrotliEncoderParams::default();
    params.quality = 11; // Maximum compression level (0-11)
    params.lgwin = 24; // Maximum window size (10-24)
    // params.lgblock = 0; // Use default block size (if applicable)
    // Removed unsupported fields:
    // params.disable_literal_context_modeling = false;
    // params.enable_transforms = true;
    // params.transform_bits = 16; 
    // params.enable_dictionary = false;

    // Compress the pixel data using Brotli with the configured parameters
    let mut compressed_data = Vec::new();
    BrotliCompress(&mut &pixel_data[..], &mut compressed_data, &params)?;

    // Create the header
    let mut header = Vec::new();
    header.extend_from_slice(MAGIC); // Magic number
    header.extend_from_slice(&width.to_be_bytes()); // Width
    header.extend_from_slice(&height.to_be_bytes()); // Height
    header.push(channels as u8); // Channels
    header.push(COMPRESSION_METHOD_BROTLI); // Compression method

    // Write header and compressed data to the output file
    let mut output_file = File::create(output_path)?;
    output_file.write_all(&header)?;
    output_file.write_all(&compressed_data)?;

    println!("Image encoded to {:?} successfully.", output_path);
    Ok(())
}

/// Decode a QP image to a standard image format
fn decode_image(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_file = File::open(input_path)?;

    // Read the header (4 + 4 + 4 + 1 + 1 = 14 bytes)
    let mut header = [0u8; 14];
    input_file.read_exact(&mut header)?;

    // Parse the header
    if &header[0..4] != MAGIC {
        return Err("Invalid QP image file: Incorrect magic number.".into());
    }

    let width = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let height = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    let channels = header[12];
    let compression_method = header[13];

    if compression_method != COMPRESSION_METHOD_BROTLI {
        return Err(format!(
            "Unsupported compression method: {}",
            compression_method
        )
        .into());
    }

    // Read the rest of the file (compressed data)
    let mut compressed_data = Vec::new();
    input_file.read_to_end(&mut compressed_data)?;

    // Decompress the pixel data using Brotli
    let mut decompressed_data = Vec::new();
    BrotliDecompress(&mut &compressed_data[..], &mut decompressed_data)?;

    // Reconstruct the image
    let img = ImageBuffer::from_raw(width, height, decompressed_data.clone())
        .ok_or("Failed to reconstruct image from pixel data.")?;

    let dynamic_image = match channels {
        3 => {
            // If the image was RGB
            let rgb_data = convert_rgba_to_rgb(&decompressed_data);
            DynamicImage::ImageRgb8(
                ImageBuffer::from_raw(width, height, rgb_data)
                    .ok_or("Failed to create RGB image.")?,
            )
        }
        4 => DynamicImage::ImageRgba8(img),
        _ => return Err("Unsupported number of channels.".into()),
    };

    // Determine the output format based on the file extension
    let output_format = match output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext_str| ext_str.to_lowercase())
    {
        Some(ref ext_lower) if ext_lower == "png" => ImageFormat::Png,
        Some(ref ext_lower) if ext_lower == "jpg" || ext_lower == "jpeg" => ImageFormat::Jpeg,
        Some(ref ext_lower) if ext_lower == "bmp" => ImageFormat::Bmp,
        Some(ref ext_lower) if ext_lower == "gif" => ImageFormat::Gif,
        Some(ref ext_lower) if ext_lower == "tiff" => ImageFormat::Tiff,
        Some(ref ext_lower) if ext_lower == "ico" => ImageFormat::Ico,
        Some(ref ext_lower) if ext_lower == "tga" => ImageFormat::Tga,
        Some(ref ext_lower) if ext_lower == "webp" => ImageFormat::WebP,
        _ => return Err("Unsupported or missing output image format extension.".into()),
    };

    // Save the image
    dynamic_image.save_with_format(output_path, output_format)?;

    println!("QP image decoded to {:?} successfully.", output_path);
    Ok(())
}

/// Helper function to convert RGBA data to RGB by removing the alpha channel
fn convert_rgba_to_rgb(rgba_data: &[u8]) -> Vec<u8> {
    rgba_data
        .chunks(4)
        .flat_map(|chunk| chunk.iter().take(3))
        .cloned()
        .collect()
}
