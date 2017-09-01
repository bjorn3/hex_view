extern crate itertools;
extern crate byteorder;
extern crate termion;
extern crate chrono;

extern crate hex_view;

use byteorder::{BigEndian, LittleEndian, ByteOrder};

use hex_view::*;
use hex_view::Color::*;

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

    let mut term_printer = TermPrinter::new(buf);
    pcapng_styler(term_printer.style_builder());
    term_printer.print();

    //let mut html_printer = HtmlPrinter::new(buf);
    //pcapng_styler(html_printer.style_builder());
    //html_printer.print();
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
        //println!("Len: {}", len);
        pcapng_block_styler(builder.block(begin, begin + len as usize, Ty::Ascii));

        begin += len as usize;
        if begin >= builder.buf.len() {
            break;
        }
    }
}

fn pcapng_block_styler(mut builder: StyleBuilder) {
    use chrono::offset::TimeZone;

    let buf = builder.buf;

    let custom = Ty::custom;
    let type_id = LittleEndian::read_u32(&buf[0..4]);
    let type_ty = match type_id {
        0x0A0D0D0A => custom("header"),
        0x1 => custom("iface descr"),
        0x2 => custom("packet"),
        0x3 => custom("simple packet"),
        0x4 => custom("name resolution"),
        0x5 => custom("iface statistics"),
        0x6 => custom("enhanced block"),
        _ => Ty::LeNum,
    };
    builder.line(4, type_ty, "type");
    builder.line(4, Ty::LeNum, "size");


    builder.set_color(White);

    match type_id {
        0x1 => {
            builder.set_color(Green);
            builder.line(2, Ty::LeNum, "link type");
            builder.line(2, Ty::Binary, "reserved");
            builder.line(4, Ty::LeNum, "snap num");
            builder.set_color(Yellow);

            let mut offset = 16 as usize;
            for i in 0.. {
                let opt1_type_num = LittleEndian::read_u16(&buf[offset..offset + 2]);
                let opt1_type = match opt1_type_num {
                    0 => "end of opts",
                    2 => "name",
                    3 => "descr",
                    4 => "ipv4 addr",
                    5 => "ipv6 addr",
                    9 => "tmstamp res",
                    12 => "OS",
                    _ => {
                        builder.set_color(Red);
                        "<unknown>"
                    },
                };
                builder.line(2, Ty::custom(opt1_type), format!("opt{} type", i));
                builder.set_color(Yellow);

                let opt1_len = LittleEndian::read_u16(&buf[offset + 2..offset + 4]) as usize;
                let opt1_len = opt1_len +
                               match opt1_len % 4 {
                                   0 => 0,
                                   1 => 3,
                                   2 => 2,
                                   3 => 1,
                                   9 => 1,
                                   _ => unreachable!(),
                               };
                let opt1_len_adapted = match opt1_type_num {
                    0 => 0, // end of opts
                    4 => 8,
                    5 => 17,
                    6 => 6,
                    7 => 8,
                    _ => opt1_len,
                };
                assert_eq!(opt1_len, opt1_len_adapted, "Opt len is wrong");
                builder.line(2, Ty::LeNum, format!("opt{}  len", i));
                builder.line(opt1_len, Ty::Ascii, format!("opt{} data", i));
                offset += 4 + opt1_len;
                if offset >= buf.len() - 4 {
                    break;
                }
                //break;
            }
            builder.line_until(buf.len() - 4, Ty::Ascii, "options");
        }
        0x6 => {
            builder.set_color(Magenta);
            builder.line(4, Ty::LeNum, "iface id");
            let timestamp = LittleEndian::read_u64(&buf[12..20]);
            println!("{}", timestamp);
            builder.line(8,
                         Ty::Custom(/*chrono::Utc.timestamp(timestamp as i64, 0)).to_rfc2822()*/
                                    "".to_string()),
                         "timestamp");
            builder.line(4, Ty::LeNum, "cap len");
            builder.line(4, Ty::LeNum, "orig len");
            builder.line(6, Ty::Binary, "dest mac");
            builder.line(6, Ty::Binary, "src mac");
            let eth_type_num = BigEndian::read_u16(&buf[40..42]);
            let eth_type = match eth_type_num {
                0x0800 => "IPv4",
                0x0806 => "ARP",
                0x0842 => "Wake-on-LAN",
                0x22F3 => "IETF TRILL Protocol",
                0x22EA => "Stream Reservation Protocol",
                0x6003 => "DECnet phase IV",
                0x86DD => "IPv6",
                _ => {
                    builder.set_color(Red);
                    "<unknown>"
                },
            };
            builder.line(2, Ty::custom(eth_type), "eth type");
            builder.set_color(Magenta);

            if eth_type_num == 0x0800 {
                builder.set_color(Cyan);
                builder.line(1, Ty::Binary, "version + IHL");
                builder.line(1, Ty::Binary, "DSCP + ECN");
                builder.line(2, Ty::BeNum, "total length");
                builder.line(2, Ty::Binary, "identification");
                builder.line(2, Ty::Binary, "flags + frag offset");
                builder.line(1, Ty::BeNum, "TTL");
                let proto_num = buf[51];
                let proto = match proto_num {
                    0x06 => "TCP",
                    0x11 => "UDP",
                    _ => "<unknown>",
                };
                builder.line(1, Ty::custom(proto), "Proto");
                builder.line(2, Ty::Binary, "Header Checksum");
                builder.line(4, Ty::Ip4, "src IP");
                builder.line(4, Ty::Ip4, "dst IP");
                match proto_num {
                    0x06 => {
                        builder.set_color(Yellow);
                        builder.line(2, Ty::BeNum, "src port");
                        builder.line(2, Ty::BeNum, "dst port");
                        builder.line(4, Ty::BeNum, "seq num");
                        builder.line(4, Ty::BeNum, "ack num");
                        builder.line(4, Ty::Binary, "data offset + opts");
                        builder.line(4, Ty::BeNum, "window size");
                        builder.line(2, Ty::Binary, "checksum");
                        builder.line(2, Ty::Binary, "urgent ptr");

                        builder.set_color(White);
                        builder.line_until(buf.len() - 4, Ty::Ascii, "content");
                    }
                    _ => {
                        builder.set_color(White);
                        builder.line_until(buf.len() - 4, Ty::Ascii, "content")
                    }
                }
            } else {
                builder.set_color(White);
                builder.line_until(buf.len() - 4, Ty::Ascii, "content");
            }
        }
        _ => builder.line_until(buf.len() - 4, Ty::Ascii, "content"),
    }

    builder.line(4, Ty::LeNum, "size");
}

/*#[allow(dead_code)]
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
}*/
