use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use clap::Parser;
use flate2::read::GzDecoder;
use gzp::{Compression, ZBuilder, deflate::Gzip};

#[derive(Parser)]
#[command(about = "Compress or decompress files using gzip format")]
pub struct Args {
    /// Input file (default: stdin)
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (default: stdout for decompress, input.gz for compress)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Decompress mode (default: compress)
    #[arg(short, long)]
    pub decompress: bool,

    /// Compression level (1-9, default: 6)
    #[arg(short = 'l', long, default_value = "6")]
    pub level: u32,

    /// Force overwrite of output file
    #[arg(short, long)]
    pub force: bool,

    /// Keep input file (don't delete after compression/decompression)
    #[arg(short, long)]
    pub keep: bool,

    /// Number of threads for compression (default: num_cpus)
    #[arg(short, long)]
    pub threads: Option<usize>,

    /// Force use of built-in implementation (don't use pigz)
    #[arg(long)]
    pub no_pigz: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Check if pigz is available and executable
fn find_pigz() -> Option<PathBuf> {
    // Check common paths
    let paths = [
        "/usr/bin/pigz",
        "/usr/local/bin/pigz",
        "/opt/homebrew/bin/pigz",
    ];

    for path in &paths {
        let p = Path::new(path);
        if p.is_file() && is_executable(p) {
            return Some(PathBuf::from(path));
        }
    }

    // Try to find in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let pigz_path = Path::new(dir).join("pigz");
            if pigz_path.is_file() && is_executable(&pigz_path) {
                return Some(pigz_path);
            }
        }
    }

    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

/// Run pigz command
fn run_pigz(
    pigz_path: &Path,
    input_path: Option<&str>,
    output_path: Option<&str>,
    decompress: bool,
    level: u32,
    threads: Option<usize>,
    keep: bool,
    force: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    if verbose {
        eprintln!("Using pigz: {}", pigz_path.display());
    }

    let mut cmd = Command::new(pigz_path);

    if decompress {
        cmd.arg("-d");
    }

    cmd.arg(format!("-{}", level.clamp(1, 9)));

    if let Some(t) = threads {
        cmd.arg("-p").arg(t.to_string());
    }

    if keep {
        cmd.arg("-k");
    }

    if force {
        cmd.arg("-f");
    }

    match (input_path, output_path) {
        (Some(input), Some(output)) => {
            cmd.arg("-c").arg(input).stdout(Stdio::piped());
            let output = std::fs::File::create(output)?;
            run_command_with_output(cmd, output)?;
        }
        (Some(input), None) => {
            cmd.arg(input);
            run_command_simple(cmd)?;
        }
        (None, Some(output)) => {
            cmd.arg("-c").stdin(Stdio::piped()).stdout(Stdio::piped());
            let output = std::fs::File::create(output)?;
            run_command_with_stdin_and_output(cmd, std::io::stdin(), output)?;
        }
        (None, None) => {
            cmd.arg("-c").stdin(Stdio::piped()).stdout(Stdio::piped());
            run_command_with_stdin(cmd, std::io::stdin())?;
        }
    }

    Ok(())
}

fn run_command_simple(mut cmd: Command) -> anyhow::Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("pigz failed with exit code: {:?}", status.code());
    }
    Ok(())
}

fn run_command_with_output(mut cmd: Command, mut output: std::fs::File) -> anyhow::Result<()> {
    let mut child = cmd.spawn()?;

    if let Some(mut stdout) = child.stdout.take() {
        std::io::copy(&mut stdout, &mut output)?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("pigz failed with exit code: {:?}", status.code());
    }
    Ok(())
}

fn run_command_with_stdin<T: Read>(mut cmd: Command, mut stdin_input: T) -> anyhow::Result<()> {
    let mut child = cmd.spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        std::io::copy(&mut stdin_input, &mut stdin)?;
    }

    if let Some(mut stdout) = child.stdout.take() {
        std::io::copy(&mut stdout, &mut std::io::stdout())?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("pigz failed with exit code: {:?}", status.code());
    }
    Ok(())
}

fn run_command_with_stdin_and_output<T: Read>(
    mut cmd: Command,
    mut stdin_input: T,
    mut output: std::fs::File,
) -> anyhow::Result<()> {
    let mut child = cmd.spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        std::io::copy(&mut stdin_input, &mut stdin)?;
    }

    if let Some(mut stdout) = child.stdout.take() {
        std::io::copy(&mut stdout, &mut output)?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("pigz failed with exit code: {:?}", status.code());
    }
    Ok(())
}

/// Built-in compression using gzp (parallel)
fn compress_builtin(
    input_path: Option<&str>,
    output_path: Option<&str>,
    level: u32,
    threads: Option<usize>,
    keep: bool,
) -> anyhow::Result<()> {
    let level = level.clamp(1, 9);
    let num_threads = threads.unwrap_or_else(num_cpus::get);
    let compression = Compression::new(level);

    // Determine input source
    let mut reader: Box<dyn Read> = match input_path {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(std::io::stdin()),
    };

    // Determine output destination
    match output_path {
        Some(path) => {
            let output_file = std::fs::File::create(path)?;
            compress_to_writer(&mut reader, output_file, compression, num_threads)?;
        }
        None => match input_path {
            Some(path) => {
                let output_path = format!("{}.gz", path);
                let output_file = std::fs::File::create(&output_path)?;
                compress_to_writer(&mut reader, output_file, compression, num_threads)?;
            }
            None => {
                let stdout = std::io::stdout();
                compress_to_writer(&mut reader, stdout, compression, num_threads)?;
                return Ok(());
            }
        },
    };

    // Delete input file if not keeping and not stdin
    if !keep {
        if let Some(path) = input_path {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn compress_to_writer<W: Write + Send + 'static>(
    reader: &mut dyn Read,
    writer: W,
    compression: Compression,
    num_threads: usize,
) -> anyhow::Result<()> {
    if num_threads <= 1 {
        let mut encoder = ZBuilder::<Gzip, _>::new()
            .num_threads(0)
            .compression_level(compression)
            .from_writer(writer);
        std::io::copy(reader, &mut encoder)?;
        encoder.finish()?;
    } else {
        let mut encoder = ZBuilder::<Gzip, _>::new()
            .num_threads(num_threads)
            .compression_level(compression)
            .from_writer(writer);
        std::io::copy(reader, &mut encoder)?;
        encoder.finish()?;
    }
    Ok(())
}

/// Built-in decompression using flate2
fn decompress_builtin(
    input_path: Option<&str>,
    output_path: Option<&str>,
    keep: bool,
) -> anyhow::Result<()> {
    let reader: Box<dyn Read> = match input_path {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(std::io::stdin()),
    };

    let mut decoder = GzDecoder::new(reader);

    let mut writer: Box<dyn Write> = match output_path {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => match input_path {
            Some(path) => {
                if path.ends_with(".gz") || path.ends_with(".gzip") {
                    let out_path = &path[..path.rfind('.').unwrap_or(path.len())];
                    Box::new(std::fs::File::create(out_path)?)
                } else {
                    let out_path = format!("{}.out", path);
                    Box::new(std::fs::File::create(&out_path)?)
                }
            }
            None => Box::new(std::io::stdout()),
        },
    };

    std::io::copy(&mut decoder, &mut writer)?;

    if !keep {
        if let Some(path) = input_path {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<()> {
    if let Some(ref output) = args.output {
        if std::path::Path::new(output).exists() && !args.force {
            anyhow::bail!(
                "Output file '{}' already exists. Use -f to force overwrite.",
                output
            );
        }
    }

    if !args.no_pigz {
        if let Some(pigz_path) = find_pigz() {
            return run_pigz(
                &pigz_path,
                args.input.as_deref(),
                args.output.as_deref(),
                args.decompress,
                args.level,
                args.threads,
                args.keep,
                args.force,
                args.verbose,
            );
        }
    }

    if args.verbose {
        eprintln!(
            "Using gzp (built-in) with {} threads",
            args.threads.unwrap_or_else(num_cpus::get)
        );
    }

    if args.decompress {
        decompress_builtin(args.input.as_deref(), args.output.as_deref(), args.keep)
    } else {
        compress_builtin(
            args.input.as_deref(),
            args.output.as_deref(),
            args.level,
            args.threads,
            args.keep,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.txt");
        let compressed = dir.path().join("test.txt.gz");
        let output = dir.path().join("output.txt");

        fs::write(&input, "Hello, World!").unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(compressed.to_string_lossy().to_string()),
            decompress: false,
            level: 6,
            force: false,
            keep: true,
            threads: Some(1),
            no_pigz: true,
            verbose: false,
        })
        .unwrap();

        assert!(compressed.exists());

        run(Args {
            input: Some(compressed.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            decompress: true,
            level: 6,
            force: false,
            keep: true,
            threads: Some(1),
            no_pigz: true,
            verbose: false,
        })
        .unwrap();

        let content = fs::read_to_string(&output).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_find_pigz() {
        let _ = find_pigz();
    }

    #[test]
    fn test_parallel_compress() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.txt");
        let compressed = dir.path().join("test.txt.gz");
        let output = dir.path().join("output.txt");

        let content = "Hello, World!\n".repeat(1000);
        fs::write(&input, &content).unwrap();

        run(Args {
            input: Some(input.to_string_lossy().to_string()),
            output: Some(compressed.to_string_lossy().to_string()),
            decompress: false,
            level: 6,
            force: false,
            keep: true,
            threads: Some(4),
            no_pigz: true,
            verbose: false,
        })
        .unwrap();

        assert!(compressed.exists());

        run(Args {
            input: Some(compressed.to_string_lossy().to_string()),
            output: Some(output.to_string_lossy().to_string()),
            decompress: true,
            level: 6,
            force: false,
            keep: true,
            threads: Some(4),
            no_pigz: true,
            verbose: false,
        })
        .unwrap();

        let decompressed_content = fs::read_to_string(&output).unwrap();
        assert_eq!(decompressed_content, content);
    }
}
