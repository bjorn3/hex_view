extern crate itertools;
extern crate byteorder;
extern crate termion;

use std::collections::HashMap;
use itertools::Itertools;
use byteorder::{BigEndian, LittleEndian, ByteOrder};
use termion::color::*;

struct TermPrinter {
    buf: Vec<u8>,
    main: Segment,
}

#[derive(Clone)]
struct Segment {
    ty: Ty,
    kind: SegmentKind,
    childs: HashMap<(usize, usize), Segment>,
}

#[derive(Copy, Clone)]
enum SegmentKind {
    Main,
    Header,
    Block,
    Line,
}

#[derive(Clone)]
enum Ty {
    Ascii,
    Binary,
    BeNum,
    LeNum,
    Custom(String),
}

impl Ty {
    fn custom(s: &str) -> Ty {
        Ty::Custom(s.to_string())
    }
}

struct StyleBuilder<'a> {
    buf: &'a [u8],
    seg: &'a mut Segment,
}

impl<'a> StyleBuilder<'a> {
    pub fn header(&mut self, begin: usize, end: usize, ty: Ty) -> StyleBuilder {
        self.seg
            .childs
            .insert((begin, end),
                    Segment {
                        ty: ty,
                        kind: SegmentKind::Header,
                        childs: HashMap::new(),
                    });
        let seg = self.seg.childs.get_mut(&(begin, end)).unwrap();
        StyleBuilder {
            buf: &self.buf[begin..end],
            seg: seg,
        }
    }

    pub fn block(&mut self, begin: usize, end: usize, ty: Ty) -> StyleBuilder {
        self.seg
            .childs
            .insert((begin, end),
                    Segment {
                        ty: ty,
                        kind: SegmentKind::Block,
                        childs: HashMap::new(),
                    });
        let seg = self.seg.childs.get_mut(&(begin, end)).unwrap();
        StyleBuilder {
            buf: &self.buf[begin..end],
            seg: seg,
        }
    }

    pub fn line(&mut self, begin: usize, end: usize, ty: Ty) {
        self.seg
            .childs
            .insert((begin, end),
                    Segment {
                        ty: ty,
                        kind: SegmentKind::Line,
                        childs: HashMap::new(),
                    });
    }
}

impl TermPrinter {
    pub fn new(buf: Vec<u8>) -> Self {
        TermPrinter {
            buf: buf,
            main: Segment {
                ty: Ty::Ascii,
                kind: SegmentKind::Main,
                childs: HashMap::new(),
            },
        }
    }

    pub fn style_builder(&mut self) -> StyleBuilder {
        StyleBuilder {
            buf: &*self.buf,
            seg: &mut self.main,
        }
    }

    fn print_hex_line(buf: &[u8]) {
        assert!(buf.len() <= 32);
        let mut num = 0;
        for b in buf.iter() {
            num += 1;
            print!("{:02X} ", b);
        }
        while num < 32 {
            num += 1;
            print!(".. ");
        }
    }

    fn make_ascii(c: char) -> char {
        match c {
            'a'...'z' | 'A'...'Z' | '0'...'9' | ':' | ';' | '@' | '/' | '\\' | '|' | '?' |
            '!' | '+' | '*' | '.' | ',' | ' ' | '-' | '_' | '\'' | '"' | '=' | '(' | ')' |
            '{' | '}' | '[' | ']' | '&' | '>' | '<' => c,
            '\n' => '␊',
            '\r' => '␍',
            '\0' => '␀',
            //c => c,
            _ => '�',
        }
    }

    pub fn print(mut self) {
        fn print_segment(buf: &[u8], s: Segment, outer_kind: SegmentKind) {
            use std::cmp::Ord;

            if s.childs.is_empty() {
                for c in buf.iter().chunks(32).into_iter() {
                    let chunk = c.cloned().collect::<Vec<u8>>();
                    match outer_kind {
                        SegmentKind::Header => print!("H "),
                        SegmentKind::Block => print!("B "),
                        _ => print!("  "),
                    }

                    TermPrinter::print_hex_line(&chunk);

                    // Print extra info
                    match s.ty {
                        Ty::Ascii => {
                            println!("  |{}{}{}|",
                                     Fg(Magenta),
                                     chunk
                                         .iter()
                                         .map(|&c| TermPrinter::make_ascii(c as char))
                                         .pad_using(32, |_| '.')
                                         .collect::<String>(),
                                     Fg(Reset))
                        }
                        Ty::Binary => println!(),
                        Ty::BeNum => {
                            let num = match buf.len() {
                                1 => buf[0] as u64,
                                2 => BigEndian::read_u16(buf) as u64,
                                4 => BigEndian::read_u32(buf) as u64,
                                8 => BigEndian::read_u64(buf) as u64,
                                len => panic!("Invalid buf len for BeNum segment: {}", len),
                            };
                            println!("  : {}{}{}", Fg(Cyan), num, Fg(Reset));
                        }
                        Ty::LeNum => {
                            let num = match buf.len() {
                                1 => buf[0] as u64,
                                2 => LittleEndian::read_u16(buf) as u64,
                                4 => LittleEndian::read_u32(buf) as u64,
                                8 => LittleEndian::read_u64(buf) as u64,
                                len => panic!("Invalid buf len for LeNum segment: {}", len),
                            };
                            println!("  : {}{}{}", Fg(Cyan), num, Fg(Reset));
                        }
                        Ty::Custom(ref custom) => {
                            println!("  ; {}{}{}", Fg(Yellow), custom, Fg(Reset));
                        }
                        _ => unreachable!(),

                    }
                }
            } else {
                let mut segments = s.childs.into_iter().collect::<Vec<_>>();
                segments.sort_by(|a, b| (a.0).0.cmp(&(b.0).0));
                for ((begin, end), seg) in segments.into_iter() {
                    let buf = &buf[begin..end];
                    print_segment(buf, seg.clone(), match (outer_kind, seg.kind) {
                        (SegmentKind::Header, _) => SegmentKind::Header,
                        (_, SegmentKind::Header) => SegmentKind::Header,
                        (SegmentKind::Block, _) => SegmentKind::Block,
                        (_, SegmentKind::Block) => SegmentKind::Block,
                        (SegmentKind::Line, _) => unreachable!(),
                        (_, SegmentKind::Line) => SegmentKind::Line,
                        _ => unreachable!(),
                    });
                    match seg.kind {
                        SegmentKind::Header | SegmentKind::Block => println!(),
                        SegmentKind::Line => {}
                        SegmentKind::Main => panic!(),
                    }
                }
            }
        }

        print_segment(&self.buf,
                      std::mem::replace(&mut self.main,
                                        Segment {
                                            ty: Ty::Ascii,
                                            kind: SegmentKind::Main,
                                            childs: HashMap::new(),
                                        }),
                      SegmentKind::Main);
    }
}

fn main() {
    use std::io::prelude::*;
    use std::fs::File;

    let mut buf = Vec::new();
    let mut file = File::open(std::env::args()
                                  .skip(1)
                                  .next()
                                  .expect("No file to view"))
            .expect("File not found");
    file.read_to_end(&mut buf).unwrap();

    let mut printer = TermPrinter::new(buf);

    pcapng_styler(printer.style_builder());

    printer.print();
}

fn pcapng_styler(mut builder: StyleBuilder) {
    let header_len = LittleEndian::read_u32(&builder.buf[4..8]);
    let header_len = header_len +
                     match header_len % 4 {
                         0 => 0,
                         1 => 3,
                         2 => 2,
                         3 => 1,
                         _ => unreachable!(),
                     };
    println!("Header len: {}", header_len);
    pcapng_block_styler(builder.header(0, header_len as usize, Ty::Ascii));

    let mut begin = header_len as usize;
    loop {
        let len = LittleEndian::read_u32(&builder.buf[begin + 4..begin + 8]);
        let len = len +
                  match len % 4 {
                      0 => 0,
                      1 => 3,
                      2 => 2,
                      3 => 1,
                      _ => unreachable!(),
                  };
        println!("Len: {}", len);
        pcapng_block_styler(builder.block(begin, begin + len as usize, Ty::Ascii));

        begin += len as usize;
        if begin >= builder.buf.len() {
            break;
        }
    }
}

fn pcapng_block_styler(mut builder: StyleBuilder) {
    let custom = Ty::custom;
    let type_ty = match LittleEndian::read_u32(&builder.buf[0..4]) {
        0x0A0D0D0A => custom("header"),
        0x1 => custom("iface descr"),
        0x2 => custom("packet"),
        0x3 => custom("simple packet"),
        0x4 => custom("name resolution"),
        0x5 => custom("iface statistics"),
        0x6 => custom("enhanced block"),
        _ => Ty::LeNum,
    };
    builder.line(0, 4, type_ty);
    builder.line(4, 8, Ty::LeNum);
    builder.line(8, builder.buf.len() - 4, Ty::Ascii);
    builder.line(builder.buf.len() - 4, builder.buf.len(), Ty::LeNum);
}

fn plain_text_styler(mut builder: StyleBuilder) {
    let mut buf_iter = builder.buf.iter().peekable();
    let mut begin = 0;
    loop {
        let mut end = begin;
        while let Some(c) = buf_iter.next() {
            end += 1;
            if *c == '\n' as u8 {
                break;
            }
        }
        builder.line(begin, end, Ty::Ascii);
        begin = end;
        if buf_iter.peek().is_none() {
            break;
        }
    }
}
