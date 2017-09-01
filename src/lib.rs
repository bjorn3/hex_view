extern crate itertools;
extern crate byteorder;
extern crate termion;

use std::collections::HashMap;
use itertools::Itertools;
use byteorder::{BigEndian, LittleEndian, ByteOrder};
use termion::color::*;

#[derive(Clone)]
struct Segment {
    ty: Ty,
    kind: SegmentKind,
    childs: HashMap<(usize, usize), Segment>,
}

#[derive(Clone)]
pub enum SegmentKind {
    Main,
    Header,
    Block,
    Line { tag: String, color: Color },
}

#[derive(Clone)]
pub enum Ty {
    Ascii,
    Binary,
    BeNum,
    LeNum,
    Ip4,
    Custom(String),
}

impl Ty {
    pub fn custom(s: &str) -> Ty {
        Ty::Custom(s.to_string())
    }
}

#[derive(Copy, Clone)]
pub enum Color {
    Blue,
    Cyan,
    Green,
    Magenta,
    Red,
    Yellow,
    White,
}

pub struct StyleBuilder<'a> {
    pub buf: &'a [u8],
    childs: &'a mut HashMap<(usize, usize), Segment>,
    part_color: Color,
    index: usize,
}

impl<'a> StyleBuilder<'a> {
    pub fn set_color(&mut self, color: Color){
        self.part_color = color;
    }
    pub fn header(&mut self, begin: usize, end: usize, ty: Ty) -> StyleBuilder {
        self.childs.insert(
            (begin, end),
            Segment {
                ty: ty,
                kind: SegmentKind::Header,
                childs: HashMap::new(),
            },
        );
        let seg = self.childs.get_mut(&(begin, end)).unwrap();
        StyleBuilder {
            buf: &self.buf[begin..end],
            childs: &mut seg.childs,
            part_color: Color::White,
            index: 0,
        }
    }

    pub fn block(&mut self, begin: usize, end: usize, ty: Ty) -> StyleBuilder {
        self.childs.insert(
            (begin, end),
            Segment {
                ty: ty,
                kind: SegmentKind::Block,
                childs: HashMap::new(),
            },
        );
        let seg = self.childs.get_mut(&(begin, end)).unwrap();
        StyleBuilder {
            buf: &self.buf[begin..end],
            childs: &mut seg.childs,
            part_color: Color::White,
            index: 0,
        }
    }
    
    pub fn index(&self) -> usize { self.index }
    
    pub fn line<S: Into<String>>(&mut self, len: usize, ty: Ty, tag: S) {
        assert!(self.index + len <= self.buf.len(), "Too big len {} exceeds buf len {} (index {})", len, self.buf.len(), self.index);
        self.childs.insert(
            (self.index, self.index + len),
            Segment {
                ty: ty,
                kind: SegmentKind::Line { tag: tag.into(), color: self.part_color },
                childs: HashMap::new(),
            },
        );
        self.index += len;
    }
    
    pub fn line_until<S: Into<String>>(&mut self, end: usize, ty: Ty, tag: S) {
        assert!(self.index <= end, "Index {} bigger than end {}", self.index, end);
        self.childs.insert(
            (self.index, end),
            Segment {
                ty: ty,
                kind: SegmentKind::Line { tag: tag.into(), color: self.part_color },
                childs: HashMap::new(),
            },
        );
        self.index = end;
    }
}

fn make_ascii(c: char) -> char {
    match c {
        // _ if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() => c,
        'a'...'z' | 'A'...'Z' | '0'...'9' | ':' | ';' | '@' | '/' | '\\' | '|' | '?' | '!' |
        '+' | '*' | '.' | ',' | ' ' | '-' | '_' | '\'' | '"' | '=' | '(' | ')' | '{' | '}' |
        '[' | ']' | '&' | '>' | '<' => c,
        '\n' => '␊',
        '\r' => '␍',
        '\0' => '␀',
        //c => c,
        _ => '�',
    }
}

fn read_num<E: ByteOrder>(buf: &[u8]) -> u64 {
    match buf.len() {
        1 => buf[0] as u64,
        2 => E::read_u16(buf) as u64,
        4 => E::read_u32(buf) as u64,
        8 => E::read_u64(buf) as u64,
        len => panic!("Invalid buf len for **Num segment: {}", len),
    }
}

pub struct TermPrinter {
    buf: Vec<u8>,
    main: Segment,
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
            childs: &mut self.main.childs,
            part_color: Color::White,
            index: 0,
        }
    }

    fn print_hex_line(buf: &[u8]) {
        assert!(buf.len() <= 32);
        let mut num = 0;
        for b in buf.iter() {
            if num % 8 == 0 {
                print!(" ");
            }
            num += 1;
            print!("{:02X} ", b);
        }
        while num < 32 {
            if num == 16 || num == 24 {
                break;
            }
            if num % 8 == 0 {
                print!(" ");
            }
            num += 1;
            print!(".. ");
        }
    }

    fn print_extras(chunk: &[u8], seg: &Segment) {
        match seg.ty {
            Ty::Ascii => {
                let text_iter = chunk
                    .iter()
                    .map(|&c| make_ascii(c as char))
                    .pad_using(32, |_| '.')
                    .enumerate();
                let mut text = String::with_capacity(40);
                for (i, c) in text_iter {
                    text.push(c);
                    if c == '�' {
                        text.push(' ');
                    }
                    if (i + 1) % 8 == 0 {
                        text.push(' ');
                    }
                    if i == 15 {
                        text.push(' ');
                    }
                }
                print!("| {}|", text)
            }
            Ty::Binary => {}
            Ty::BeNum => {
                print!(": {}", read_num::<BigEndian>(chunk));
            }
            Ty::LeNum => {
                print!(": {}", read_num::<LittleEndian>(chunk));
            }
            Ty::Ip4 => {
                assert!(chunk.len() == 4, "Wrong len for ipv4 addr");
                print!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
            }
            Ty::Custom(ref custom) => {
                let num = match chunk.len() {
                    1 | 2 | 4 | 8 => Some(read_num::<LittleEndian>(chunk)),
                    _ => None,
                };
                print!("; {} ({})", custom, num.map(|n|n.to_string()).as_ref().map(|s|s as &str).unwrap_or(""));
            }
        }
    }

    fn print_segment(buf: &[u8], s: Segment) {
        use std::cmp::Ord;

        if s.childs.is_empty() {
            for c in buf.iter().chunks(32).into_iter() {
                let chunk = c.cloned().collect::<Vec<u8>>();

                match s.kind {
                    SegmentKind::Line { ref tag, color } => {
                        match color {
                            Color::Blue => print!("{}", Fg(Blue)),
                            Color::Cyan => print!("{}", Fg(Cyan)),
                            Color::Green => print!("{}", Fg(Green)),
                            Color::Magenta => print!("{}", Fg(Magenta)),
                            Color::Red => print!("{}", Fg(Red)),
                            Color::Yellow => print!("{}", Fg(Yellow)),
                            Color::White => print!("{}", Fg(White)),
                        }
                        TermPrinter::print_hex_line(&chunk);
                        print!("  {:>12} ", tag);
                    }
                    _ => {
                        TermPrinter::print_hex_line(&chunk);
                        print!("          ");
                    }
                }
                TermPrinter::print_extras(&chunk, &s);
                println!("{}", Fg(Reset));
            }
        } else {
            let mut segments = s.childs.into_iter().collect::<Vec<_>>();
            segments.sort_by(|a, b| (a.0).0.cmp(&(b.0).0));

            for ((begin, end), seg) in segments.into_iter() {
                let buf = &buf[begin..end];
                TermPrinter::print_segment(buf, seg.clone());
                match seg.kind {
                    SegmentKind::Header | SegmentKind::Block => println!(),
                    SegmentKind::Line { .. } => {}
                    SegmentKind::Main => panic!(),
                }
            }
        }
    }

    pub fn print(self) {
        let TermPrinter { buf, main } = self;
        TermPrinter::print_segment(&buf, main);
    }
}

/*pub struct HtmlPrinter {
    buf: Vec<u8>,
    main: Segment,
}

impl HtmlPrinter {
    pub fn new(buf: Vec<u8>) -> Self {
        HtmlPrinter {
            buf: buf,
            main: Segment {
                ty: Ty::Ascii,
                tag: "".to_string(),
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
    
    fn print_hex_line(buf: &[u8]) {
        assert!(buf.len() <= 32);
        print!("<code class='hex'>");
        let mut num = 0;
        for b in buf.iter() {
            if num == 16 {
                print!(" ");
            } else if num % 8 == 0 {
                print!(" ");
            }
            num += 1;
            print!("{:02X} ", b);
        }
        print!("</code>");
    }
    
    fn print_extras(chunk: &[u8], seg: &Segment) {
        print!("<code>");
        match seg.ty {
            Ty::Ascii => {
                let text_iter = chunk
                    .iter()
                    .map(|&c| HtmlPrinter::make_ascii(c as char))
                    .pad_using(32, |_| '.')
                    .enumerate();
                let mut text = String::with_capacity(40);
                for (i, c) in text_iter {
                    text.push(c);
                    if (i + 1) % 8 == 0 {
                        text.push(' ');
                    }
                    if i == 15 {
                        text.push(' ');
                    }
                }
                print!("|{}|", text)
            }
            Ty::Binary => {},
            Ty::BeNum => {
                let num = match chunk.len() {
                    1 => chunk[0] as u64,
                    2 => BigEndian::read_u16(chunk) as u64,
                    4 => BigEndian::read_u32(chunk) as u64,
                    8 => BigEndian::read_u64(chunk) as u64,
                    len => panic!("Invalid buf len for BeNum segment: {}", len),
                };
                print!(": {}", num);
            }
            Ty::LeNum => {
                let num = match chunk.len() {
                    1 => chunk[0] as u64,
                    2 => LittleEndian::read_u16(chunk) as u64,
                    4 => LittleEndian::read_u32(chunk) as u64,
                    8 => LittleEndian::read_u64(chunk) as u64,
                    len => panic!("Invalid buf len for LeNum segment: {}", len),
                };
                print!(": {}", num);
            }
            Ty::Custom(ref custom) => {
                print!("; {}", custom);
            }
        }
        print!("</code>");
    }
    
    fn print_segment(buf: &[u8], s: Segment) {
        use std::cmp::Ord;

        if s.childs.is_empty() {
            for c in buf.iter().chunks(32).into_iter() {
                let chunk = c.cloned().collect::<Vec<u8>>();
                //match s.ty {
                //    Ty::Ascii => print!("{}", Fg(Magenta)),
                //    Ty::Binary => print!("{}", Fg(Reset)),
                //    Ty::BeNum | Ty::LeNum => print!("{}", Fg(Cyan)),
                //    Ty::Custom(_) => print!("{}", Fg(Yellow)),
                //}
                print!("<div class='line' style='color: {}'>", match s.ty {
                    Ty::Ascii => "magenta",
                    Ty::Binary => "",
                    Ty::BeNum | Ty::LeNum => "cyan",
                    Ty::Custom(_) => "yellow",
                });
                HtmlPrinter::print_hex_line(&chunk);
                print!("<span>  {:>8} </span>", s.tag);
                HtmlPrinter::print_extras(&chunk, &s);
                println!("</div>");
                //print!("{}", Fg(Reset));
            }
        } else {
            let mut segments = s.childs.into_iter().collect::<Vec<_>>();
            segments.sort_by(|a, b| (a.0).0.cmp(&(b.0).0));

            for ((begin, end), seg) in segments.into_iter() {
                let buf = &buf[begin..end];
                HtmlPrinter::print_segment(buf, seg.clone());
                match seg.kind {
                    SegmentKind::Header | SegmentKind::Block => println!("<br>"),
                    SegmentKind::Line => {}
                    SegmentKind::Main => panic!(),
                }
            }
        }
    }

    pub fn print(mut self) {
        println!("<html>
        <head>
        <meta charset=\"utf-8\">
        <style>
body {{
    width: 2000px;
    font-family: monospace;
    font-size: 15px;
    background: black;
    color: yellowgreen;
}}
.line {{
    white-space: pre;
}}
.hex {{
    display: inline-block;
    width: 900px;
}}
        </style>
        </head>
        <body>");
        HtmlPrinter::print_segment(
            &self.buf,
            std::mem::replace(
                &mut self.main,
                Segment {
                    ty: Ty::Ascii,
                    tag: "".to_string(),
                    kind: SegmentKind::Main,
                    childs: HashMap::new(),
                },
            ),
        );
        println!("</body>
        </html>");
    }
}*/
