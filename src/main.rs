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

#[derive(Clone)]
enum SegmentKind {
    Main,
    //Header,
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
    fn is_custom_repr(&self) -> bool {
        match *self {
            Ty::BeNum | Ty::LeNum | Ty::Custom(_) => true,
            _ => false,
        }
    }
}

struct StyleBuilder<'a> {
    buf: &'a [u8],
    seg: &'a mut Segment,
}

impl<'a> StyleBuilder<'a> {
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

    pub fn print(mut self) {
        fn print_segment(buf: &[u8], s: Segment) {
            fn make_ascii(c: char) -> char {
                match c {
                    'a'...'z' | 'A'...'Z' | '0'...'9' | ':' | ';' | '@' | '/' | '\\' | '|' |
                    '?' | '!' | '+' | '*' | '.' | ',' | ' ' | '-' | '_' | '\'' | '"' | '=' |
                    '(' | ')' | '{' | '}' | '[' | ']' | '&' | '>' | '<' => c,
                    '\n' => '␊',
                    '\r' => '␍',
                    '\0' => '␀',
                    //c => c,
                    _ => '�',
                }
            }
            use std::cmp::Ord;

            if s.childs.is_empty() {
                if !s.ty.is_custom_repr() {
                    for c in buf.iter().chunks(32).into_iter() {
                        let chunk = c.cloned().collect::<Vec<u8>>();
                        TermPrinter::print_hex_line(&chunk);

                        match s.ty {
                            Ty::Ascii => {
                                println!("  |{}{}{}|",
                                         Fg(Magenta),
                                         chunk
                                             .iter()
                                             .map(|&c| make_ascii(c as char))
                                             .pad_using(32, |_| '.')
                                             .collect::<String>(),
                                         Fg(Reset))
                            }
                            Ty::Binary => println!(),
                            _ => unreachable!(),
                        }
                    }
                } else {
                    TermPrinter::print_hex_line(buf);

                    match s.ty {
                        Ty::BeNum => {
                            let num = match buf.len() {
                                1 => buf[0] as u64,
                                2 => BigEndian::read_u16(buf) as u64,
                                4 => BigEndian::read_u32(buf) as u64,
                                8 => BigEndian::read_u64(buf) as u64,
                                len => panic!("Invalid buf len for BeNum segment"),
                            };
                            println!("  : {}{}{}", Fg(Cyan), num, Fg(Reset));
                        }
                        Ty::LeNum => {
                            let num = match buf.len() {
                                1 => buf[0] as u64,
                                2 => LittleEndian::read_u16(buf) as u64,
                                4 => LittleEndian::read_u32(buf) as u64,
                                8 => LittleEndian::read_u64(buf) as u64,
                                len => panic!("Invalid buf len for LeNum segment"),
                            };
                            println!("  : {}{}{}", Fg(Cyan), num, Fg(Reset));
                        }
                        Ty::Custom(custom) => {
                            println!("  : {}{}{}", Fg(Yellow), custom, Fg(Reset));
                        }
                        _ => unreachable!(),
                    }
                }
            } else {
                let mut segments = s.childs.into_iter().collect::<Vec<_>>();
                segments.sort_by(|a, b| (a.0).0.cmp(&(b.0).0));
                for ((begin, end), seg) in segments.into_iter() {
                    let buf = &buf[begin..end];
                    print_segment(buf, seg.clone());
                    match seg.kind {
                        SegmentKind::Block => println!(),
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
                                        }));
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
    let mut begin = 0;
    loop {
        let len = LittleEndian::read_u32(&builder.buf[begin + 4..begin + 8]);
        println!("Len: {}", len);
        pcapng_block_styler(builder.block(begin, begin + len as usize, Ty::Ascii));

        begin += len as usize;
        if begin >= builder.buf.len() {
            break;
        }
    }
}

fn pcapng_block_styler(mut builder: StyleBuilder) {
    let type_ty = match LittleEndian::read_u32(&builder.buf[0..4]) {
        0x0A0D0D0A => Ty::Custom("header".to_string()),
        0x00000001 => Ty::Custom("iface descr".to_string()),
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
