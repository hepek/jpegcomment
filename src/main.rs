use std::io::{Write, BufWriter};
use clap::Parser;

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

#[derive(Debug)]
enum JpegError {
    BufferTooShort
}

impl std::fmt::Display for JpegError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str("JpegEror")
    }
}
impl std::error::Error for JpegError {
}

#[derive(Clone)]
enum JpegElement <'a> {
    Soi,
    Seg(u8, &'a [u8]),
    Comment(&'a [u8]),
    ECS(&'a [u8]),
    Restart(u8),
    Eoi,
}

impl<'a> JpegElement<'a> {
    fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            JpegElement::Soi => writer.write_all(&[0xff, 0xd8])?,
            JpegElement::Eoi => writer.write_all(&[0xff, 0xd9])?,
            JpegElement::Restart(nr) => writer.write_all(&[0xff, *nr])?,
            JpegElement::Seg(nr, data) => {
                writer.write_all(&[0xff, *nr])?;
                let len = data.len() as u16 + 2u16;
                writer.write_all(&len.to_be_bytes())?;
                writer.write_all(data)?;
            },
            JpegElement::Comment(data) => {
                writer.write_all(&[0xff, 0xfe])?;
                let len = data.len() as u16 + 2u16;
                writer.write_all(&len.to_be_bytes())?;
                writer.write_all(data)?;
            }
            JpegElement::ECS(data) => {
                writer.write_all(data)?;
            }
        }
        Ok(())
    }
}

impl<'a> std::fmt::Debug for JpegElement<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            JpegElement::Soi => fmt.write_str("Soi\n"),
            JpegElement::Seg(nr, data) => fmt.write_str(&format!("Seg 0x{:x} {}B\n", nr, data.len())),
            JpegElement::Comment(data) => fmt.write_str(&format!("Comment: {}B\n", String::from_utf8_lossy(data))),
            JpegElement::ECS(data) => fmt.write_str(&format!("ECS: {}B\n", data.len())),
            JpegElement::Restart(nr) => fmt.write_str(&format!("Restart 0x{:x}\n", nr)),
            JpegElement::Eoi => fmt.write_str("Eoi\n"),
        }
    }
}

#[derive(Debug)]
struct Jpeg <'a> {
    elems: Vec<JpegElement<'a>>,
}

enum DecoderState {
    Init,
    SeenFF,
    InitEcs,
    SeenFFEcs,
}

impl<'a> Jpeg<'a> {
    fn deserialize(data: &'a [u8]) -> Result<Jpeg<'a>, JpegError> {
        let mut elems = vec![];
        let mut state = DecoderState::Init;
        let mut offset = 0usize;
        let size = data.len();
        let mut ecs_start = None;

        loop {
            let byte = data[offset];
            offset+=1;
            match state {
                DecoderState::Init => {
                    if byte == 0xff {
                        state = DecoderState::SeenFF;
                    }
                },
                DecoderState::SeenFF => {
                    match byte {
                        0x00 => {
                            eprintln!("unexpected ff 00");
                            state = DecoderState::Init;
                        },
                        0xd8 => { 
                            elems.push(JpegElement::Soi);
                            state = DecoderState::Init;
                        },
                        0xd9 => {
                            elems.push(JpegElement::Eoi);
                            break;
                        },
                        byte => {
                            check_read(size, offset+2)?;
                            let buf = &data[offset..offset+2];
                            offset += 2;
                            let len = (u16::from_be_bytes([buf[0], buf[1]]) - 2) as usize;
                            check_read(size, offset+len)?;
                            let data = &data[offset..offset+len];
                            offset += len;
                            if byte == 0xfe {
                                elems.push(JpegElement::Comment(data));
                            } else {
                                elems.push(JpegElement::Seg(byte, data));
                            }
                            state = DecoderState::Init;

                            if byte == 0xda { //Start of Scan
                                state = DecoderState::InitEcs;
                                ecs_start = Some(offset);
                            }
                        }
                    }
                },
                DecoderState::InitEcs => {
                    if byte == 0xff {
                        state = DecoderState::SeenFFEcs;
                    }
                },
                DecoderState::SeenFFEcs => {
                    match byte {
                        0x00 => {
                            state = DecoderState::InitEcs;
                        },
                        0xd9 => {
                            elems.push(JpegElement::ECS(&data[ecs_start.unwrap()..offset-2]));
                            elems.push(JpegElement::Eoi);
                            break;
                        },
                        byte => {
                            if (0xd0..0xd8).contains(&byte) { // restart bytes
                                elems.push(JpegElement::ECS(&data[ecs_start.unwrap()..offset-2]));
                                elems.push(JpegElement::Restart(byte));
                                state = DecoderState::InitEcs;
                                ecs_start = Some(offset);
                            }
                        },
                    }
                }
            }
        }

        Ok(Jpeg { elems })
    }

    fn serialize(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        for elem in self.elems.iter() {
            elem.write(writer)?;
        }
        Ok(())
    }

    fn delete_comment(&mut self) -> Option<&'a [u8]> {
        for i in 0..self.elems.len() {
            match self.elems[i] {
                JpegElement::Comment(data) => {
                    self.elems.remove(i);
                    return Some(data);
                },
                _ => {
                },
            }
        }

        None
    }

    fn set_comment(&mut self, data: &'a [u8]) -> Option<&'a [u8]> {
        let res = self.delete_comment();        
        let comment = JpegElement::Comment(data);
        // insert comment right after e0
        if let Some((idx, _)) = self.elems.iter()
            .enumerate()
            .find(|(_, elem)| match elem {
                JpegElement::Seg(0xe0, _) => true,
                _ => false,
            }) {
                self.elems.insert(idx+1, comment);
        } else {
            self.elems.push(comment);
        }
        res
    }
}

fn check_read(len: usize, last: usize) -> Result<(), JpegError> {
    if len < last {
        Err(JpegError::BufferTooShort)
    } else {
        Ok(())
    }
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
                JpegElement::Seg(tag, _) if (0xe0..0xe7).contains(tag) => None,
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
