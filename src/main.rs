extern crate itertools;

use std::collections::HashMap;
use itertools::Itertools;

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

#[derive(Copy, Clone)]
enum Ty {
    Ascii,
}

impl TermPrinter {
    pub fn new(buf: Vec<u8>) -> Self {
        TermPrinter {
            buf: buf,
            main: Segment {
                ty: Ty::Ascii,
                kind: SegmentKind::Main,
                childs: HashMap::new(),
            }
        }
    }
    
    pub fn print(mut self) {
        fn print_segment(buf: &[u8], s: Segment) {
            use std::cmp::Ord;
            
            if s.childs.is_empty() {
                for c in buf.iter().chunks(32).into_iter() {
                    let chunk = c.cloned().collect::<Vec<u8>>();
                    let mut num = 0;
                    for b in chunk.iter() {
                        num += 1;
                        print!("{:02X} ", b);
                    }
                    while num < 32 {
                        num += 1;
                        print!("   ");
                    }
                    
                    println!("  |{}|", String::from_utf8_lossy(&chunk).replace('\n', "␊").chars().pad_using(32, |_| '.').collect::<String>());
                }
            } else {
                let mut segments = s.childs.into_iter().collect::<Vec<_>>();
                segments.sort_by(|a,b|(a.0).0.cmp(&(b.0).0));
                for ((begin, end), seg) in segments.into_iter() {
                    let buf = &buf[begin..end];
                    print_segment(buf, seg.clone());
                    match seg.kind {
                        SegmentKind::Block => println!(),
                        SegmentKind::Line => {},
                        SegmentKind::Main => panic!(),
                    }
                }
            }
        }
        print_segment(&self.buf, std::mem::replace(&mut self.main, Segment {
            ty: Ty::Ascii,
            kind: SegmentKind::Main,
            childs: HashMap::new(),
        }));
    }
}

fn main() {
    let buf = include_bytes!("main.rs");
    let mut printer = TermPrinter::new(buf.to_vec());
    let mut buf_iter = buf.iter().peekable();
    let mut begin = 0;
    loop {
        let mut end = begin;
        while let Some(c) = buf_iter.next() {
            end += 1;
            if *c == '\n' as u8 {
                break;
            }
        }
        printer.main.childs.insert((begin, end), Segment {
            ty: Ty::Ascii,
            kind: SegmentKind::Line,
            childs: HashMap::new(),
        });
        begin = end;
        if buf_iter.peek().is_none() {
            break;
        }
    }
    
    printer.print();
}
