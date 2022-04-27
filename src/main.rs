use std::io::{Write, BufWriter};
use clap::Parser;
use jpegcomment::*;

#[derive(Parser, Debug)]
struct Args {
    /// Input jpeg file
    #[clap(short, long)]
    input: String,
    /// Output file (defaults to stdout)
    #[clap(short, long, default_value = "-")]
    output: String,
    /// Set jpeg comment
    #[clap(short, long)]
    comment: Option<String>,
    /// Delete jpeg comment
    #[clap(short)]
    delete_comment: bool,
    /// Print jpeg comment
    #[clap(short)]
    print_comment: bool,
    /// Delete all APP segments removing all image metadata
    #[clap(short, long)]
    anonymize: bool,
    /// Print jpeg structure
    #[clap(long)]
    dbgprint: bool,
}

fn open_outfile(file: &str) -> Result<Box<dyn Write>, String> {
    if file == "-" {
        Ok(Box::new(std::io::stdout()))
    } else {
        let writer = std::fs::File::create(file)
            .map_err(|e| format!("failed opening output file: {file}: {e}"))?;
        Ok(Box::new(BufWriter::new(writer)))
    }
}

fn print_comment(jpeg: &Jpeg) -> Result<(), std::io::Error> {
    for elem in jpeg.elems.iter() {
        match elem {
            JpegElement::Comment(data) => {
                std::io::stdout().write_all(&data)?;
            },
            _ => {
            },
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let file_data = std::fs::read(&args.input)
        .map_err(|e| format!("failed reading input file: {}: {}", &args.input, e))?;

    let mut jpeg = Jpeg::deserialize(&file_data)?;
    
    if args.print_comment {
        print_comment(&jpeg)?;
        return Ok(());
    }

    if args.dbgprint {
        println!("{:?}", jpeg);
        return Ok(());
    }

    if args.delete_comment {
        if let Some(old_comment) = jpeg.delete_comment() {
            let old_comment = String::from_utf8_lossy(old_comment);
            eprintln!("deleted comment: {old_comment}");
        }
    }

    if args.anonymize {
        let elems2: Vec<_> = jpeg.elems.iter()
            .filter_map(|elem| match elem {
                JpegElement::Seg(tag, _) if (0xe1..=0xef).contains(tag) => None,
                elem => Some(elem),
            })
        .cloned()
        .collect();
        jpeg.elems = elems2;
    }

    if let Some(ref comment) = args.comment {
        if let Some(old_comment) = jpeg.set_comment(&comment.as_bytes()) {
            let old_comment = String::from_utf8_lossy(old_comment);
            eprintln!("replaced comment: {old_comment}");
        }
    }

    let mut writer = open_outfile(&args.output)?;
    jpeg.serialize(&mut writer)?;

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_zeros() {
        let zeros = [0u8; 100];
        let res = jpegcomment::Jpeg::deserialize(&zeros);

        println!("{:?}", res);
    }
}
