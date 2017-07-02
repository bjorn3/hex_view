extern crate itertools;
extern crate byteorder;
extern crate termion;

extern crate hex_view;

use byteorder::{BigEndian, LittleEndian, ByteOrder};

use hex_view::*;

fn main() {
    use std::io::prelude::*;
    use std::fs::File;

    let mut buf = Vec::new();
    let mut file = File::open(std::env::args().skip(1).next().expect("No file to view"))
        .expect("File not found");
    file.read_to_end(&mut buf).unwrap();

    //let mut term_printer = TermPrinter::new(buf);
    //pcapng_styler(term_printer.style_builder());
    //term_printer.print();
    
    let mut html_printer = HtmlPrinter::new(buf);
    pcapng_styler(html_printer.style_builder());
    html_printer.print();
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
    //println!("Header len: {}", header_len);
    pcapng_block_styler(builder.header(0, header_len as usize, Ty::Ascii, "header"));

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
        //println!("Len: {}", len);
        pcapng_block_styler(builder.block(
            begin,
            begin + len as usize,
            Ty::Ascii,
            "block",
        ));

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
    builder.line(0, 4, type_ty, "type");
    builder.line(4, 8, Ty::LeNum, "size");
    builder.line(8, builder.buf.len() - 4, Ty::Ascii, "content");
    //plain_text_styler(builder.block(8, builder.buf.len() - 4, Ty::Ascii));
    builder.line(builder.buf.len() - 4, builder.buf.len(), Ty::LeNum, "size");
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
        builder.line(begin, end, Ty::Ascii, "line");
        begin = end;
        if buf_iter.peek().is_none() {
            break;
        }
    }
}
