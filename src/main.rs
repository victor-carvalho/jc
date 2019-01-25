#[macro_use]
extern crate failure;

use atty::Stream;
use clap::{App, Arg, ArgMatches};
use failure::Error;
use serde_json::{self, Value, Deserializer};
use std::fs::File;
use std::io::{self, BufRead, BufWriter, BufReader, Write};
use std::iter::Iterator;
use std::path::Path;

type Result<T> = std::result::Result<T, Error>;

struct JCArgs<'a> {
    columns: Vec<String>,
    separator: &'a str,
    show_headers: bool,
    raw: bool,
    no_root: bool,
    in_file: Option<&'a Path>,
    out_file: Option<&'a Path>,
}

impl JCArgs<'_> {
    fn from_matches<'a>(matches: &'a ArgMatches) -> JCArgs<'a> {
        JCArgs {
            columns: matches.values_of_lossy("COLUMNS").unwrap(),
            raw: matches.is_present("RAW"),
            separator: matches.value_of("SEP").unwrap(),
            show_headers: !matches.is_present("NO-HEADERS"),
            no_root: matches.is_present("NO-ROOT"),
            in_file: matches.value_of_os("INPUT").map(|s| Path::new(s)),
            out_file: matches.value_of("OUTPUT").map(|s| Path::new(s)),
        }
    }

    fn input_or<'a>(&self, stdin: &'a io::Stdin) -> Result<Box<BufRead + 'a>> {
        Ok(if let Some(f) = self.in_file {
            Box::new(BufReader::new(File::open(f)?))
        } else {
            Box::new(stdin.lock())
        })
    }

    fn output_or<'a>(&self, stdout: &'a io::Stdout) -> Result<Box<Write + 'a>> {
        Ok(if let Some(f) = self.out_file {
            Box::new(BufWriter::new(File::create(f)?))
        } else if atty::is(Stream::Stdout) {
            Box::new(stdout.lock())
        } else {
            Box::new(BufWriter::new(stdout.lock()))
        })
    }
}

fn print_line(element: &Value, args: &JCArgs, out_stream: &mut Write) -> Result<()> {
    let last_column = args.columns.len() - 1;
    match element {
        object @ Value::Object(_) => {
            for (i, col) in args.columns.iter().enumerate() {
                match &object[col] {
                    Value::String(s) => {
                        if args.raw {
                            out_stream.write(s.as_bytes())?;
                        } else {
                            write!(out_stream, "\"{}\"", s.replace("\"", "\"\""))?;
                        }
                    }
                    Value::Bool(b) => {
                        write!(out_stream, "{}", b)?;
                    }
                    Value::Number(n) => {
                        write!(out_stream, "{}", n)?;
                    }
                    Value::Null => {}
                    e => return Err(format_err!("invalid column: {}", e))
                }
                if i != last_column {
                    write!(out_stream, "{}", args.separator)?;
                }
            }
            write!(out_stream, "\n")?;
        }
        e => return Err(format_err!("invalid json object: {}", e))
    }
    Ok(())
}

fn print_header(args: &JCArgs, out_stream: &mut Write) -> Result<()> {
    if args.show_headers {
        let last_column = args.columns.len() - 1;
        for (i, col) in args.columns.iter().enumerate() {
            out_stream.write(col.as_bytes())?;
            if i != last_column {
                out_stream.write(args.separator.as_bytes())?;
            }
        }
        write!(out_stream, "\n")?;
    }
    Ok(())
}


fn run(args: JCArgs) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let in_stream = args.input_or(&stdin)?;
    let mut out_stream = args.output_or(&stdout)?;


    if args.no_root {
        let elements = Deserializer::from_reader(in_stream).into_iter::<Value>();
        print_header(&args, &mut out_stream)?;
        for result in elements {
            let value = result?;
            print_line(&value, &args, &mut out_stream)?;
        }
        Ok(())
    } else {
        match serde_json::from_reader(in_stream)? {
            Value::Array(elements) => {
                print_header(&args, &mut out_stream)?;
                elements.iter().try_for_each(|e| print_line(e, &args, &mut out_stream))
            }
            _ => Err(format_err!("root object is not an array")),
        }
    }
}

fn main() -> Result<()> {
    let matches = App::new("jc")
        .version("0.1.0")
        .author("Victor Carvalho <carvalhogvm@gmail.com>")
        .about("Convert JSON input to CSV/TSV")
        .arg(Arg::with_name("INPUT")
            .short("i")
            .long("input")
            .value_name("FILE")
            .help("Sets the input file to use"))
        .arg(Arg::with_name("OUTPUT")
            .short("o")
            .long("output")
            .value_name("FILE")
            .help("Sets the output file to use"))
        .arg(Arg::with_name("RAW")
            .short("r")
            .long("raw")
            .help("Turn off headers"))
        .arg(Arg::with_name("NO-HEADERS")
            .long("no-headers")
            .help("Turn off headers"))
        .arg(Arg::with_name("NO-ROOT")
            .long("no-root")
            .help("Turn this if json file does not have a valid root but multiple ones"))
        .arg(Arg::with_name("SEP")
            .short("s")
            .long("sep")
            .default_value(",")
            .help("Separator to be used on the output"))
        .arg(Arg::with_name("COLUMNS")
            .short("c")
            .long("columns")
            .required(true)
            .use_delimiter(true)
            .help("Columns to output"))
        .get_matches();

    run(JCArgs::from_matches(&matches))
}
