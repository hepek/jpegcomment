use std::io::Write;

#[derive(Debug)]
pub enum JpegError {
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
pub enum JpegElement <'a> {
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
pub struct Jpeg <'a> {
    pub elems: Vec<JpegElement<'a>>,
}

enum DecoderState {
    Init,
    SeenFF,
    InitEcs,
    SeenFFEcs,
}

impl<'a> Jpeg<'a> {
    pub fn deserialize(data: &'a [u8]) -> Result<Jpeg<'a>, JpegError> {
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

    pub fn serialize(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        for elem in self.elems.iter() {
            elem.write(writer)?;
        }
        Ok(())
    }

    pub fn delete_comment(&mut self) -> Option<&'a [u8]> {
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

    pub fn set_comment(&mut self, data: &'a [u8]) -> Option<&'a [u8]> {
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

