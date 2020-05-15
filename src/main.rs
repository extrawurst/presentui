use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent},
    queue,
    terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType},
    QueueableCommand, Result,
};
use figlet_rs::FIGfont;
use fs::File;
use io::stderr;
use ron::de::from_reader;
use serde::Deserialize;
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::Command,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use termimad::{Area, MadSkin};

//TODO: https://crates.io/crates/syntect

#[derive(Debug, Deserialize)]
enum FileTypes {
    Markdown(String),
    Image(String),
    GifAnimation(String),
    Open(String),
    Print(String),
    FIGlet(String),
    Code(String),
}

fn longest_line(s: &str) -> usize {
    1 + s.lines().fold(0, |acc, l| acc.max(l.len()))
}

impl FileTypes {
    fn show(&self, w: &mut impl Write, margin: usize) -> Result<()> {
        match self {
            //TODO: open the file using enter
            FileTypes::GifAnimation(path) => {
                disable_raw_mode()?;
                Command::new("viu").arg("-1").arg(path).status()?;
                enable_raw_mode()?;
            }
            FileTypes::Image(path) => {
                disable_raw_mode()?;
                let (w, h) = terminal::size()?;
                Command::new("viu")
                    .arg(format!("-w{}", w))
                    .arg(format!("-h{}", h))
                    .arg(path)
                    .status()?;
                enable_raw_mode()?;
            }
            FileTypes::Print(txt) => {
                w.write(txt.as_bytes())?;
            }
            FileTypes::Markdown(path) => {
                let (width, height) = terminal::size().unwrap();
                let markdown = fs::read_to_string(Path::new(path))?;
                let text_width = longest_line(markdown.as_str()) as u16;

                let area_w = text_width.min(width - (margin as u16 * 2));
                let area_h = height - (margin as u16 * 2);

                let x = (width - area_w) / 2;
                let y = (height - area_h) / 2;

                MadSkin::default()
                    .write_in_area(&markdown, &Area::new(x, y, area_w, area_h))
                    .unwrap();
            }
            FileTypes::Open(path) => {
                w.write(format!("opening: {}\n", path).as_bytes())?;

                Command::new("open")
                    .arg("-W")
                    .arg("-F")
                    .arg("-n")
                    .arg(path)
                    .status()?;

                w.write("closed".as_bytes())?;
            }
            FileTypes::Code(path) => {
                // let (width, height) = terminal::size().unwrap();
                let content = fs::read_to_string(Path::new(path))?;
                // let text_width = longest_line(content.as_str());

                // Load these once at the start of your program
                let ps = SyntaxSet::load_defaults_newlines();
                let ts = ThemeSet::load_defaults();

                let syntax = ps.find_syntax_by_extension("rs").unwrap();
                let mut h = HighlightLines::new(syntax, &ts.themes["Solarized (light)"]);

                for line in LinesWithEndings::from(content.as_str()) {
                    let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    w.write(format!("{}\r", escaped).as_bytes())?;
                }
            }
            FileTypes::FIGlet(txt) => {
                //TODO: draw manually to allow centering
                let standard_font = FIGfont::standand().unwrap();
                let figure = standard_font.convert(txt).unwrap();
                disable_raw_mode()?;
                w.write(format!("{}", figure).as_bytes())?;
                enable_raw_mode()?;
            }
        }

        Ok(())
    }
}

fn main_loop(w: &mut impl Write, slides: &Slides) -> Result<()> {
    let mut idx = 0_usize;
    let mut margin = 2_usize;

    loop {
        w.queue(Clear(ClearType::All))?.queue(MoveTo(0, 0))?;
        w.flush()?;

        if let Some(file) = slides.files.get(idx) {
            file.show(w, margin)?;
        } else {
            break;
        }

        w.flush()?;

        match read_input()? {
            Input::Quit => {
                break;
            }
            Input::Previous => idx = idx.saturating_sub(1),
            Input::Next => idx = idx.saturating_add(1),
            Input::Margin(plus) => {
                if plus {
                    margin = margin.saturating_add(1)
                } else {
                    margin = margin.saturating_sub(1)
                }
            }
            Input::None => (),
        }
    }

    Ok(())
}

enum Input {
    None,
    Previous,
    Next,
    Margin(bool),
    Quit,
}

fn read_input() -> Result<Input> {
    let ev = event::read()?;

    if let Event::Key(ev) = ev {
        return match ev {
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => Ok(Input::Next),
            KeyEvent {
                code: KeyCode::Up, ..
            } => Ok(Input::Previous),
            KeyEvent {
                code: KeyCode::Char('+'),
                ..
            } => Ok(Input::Margin(true)),
            KeyEvent {
                code: KeyCode::Char('-'),
                ..
            } => Ok(Input::Margin(false)),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => Ok(Input::Quit),

            _ => Ok(Input::None),
        };
    }

    Ok(Input::None)
}

#[derive(Debug, Deserialize)]
struct Slides {
    files: Vec<FileTypes>,
}

fn main() -> Result<()> {
    let f = File::open("example.ron").expect("Failed opening file");
    let slides: Slides = from_reader(f).expect("Failed to parse ron");

    // let files = &[
    //     FileTypes::Print("hello world"),
    //     FileTypes::Code("assets/test.rs"),
    //     FileTypes::Markdown("assets/test2.md"),
    //     FileTypes::Markdown("assets/test_table.md"),
    //     FileTypes::FIGlet("hello world"),
    //     FileTypes::GifAnimation("assets/giphy3.gif"),
    //     FileTypes::Markdown("test.md"),
    //     FileTypes::Image("assets/logo.png"),
    //     FileTypes::Open("assets/s00-diff.png"),
    //     FileTypes::GifAnimation("assets/giphy.gif"),
    //     FileTypes::Print("end"),
    // ];

    let mut w = stderr();

    enable_raw_mode()?;

    queue!(w, Hide)?;

    main_loop(&mut w, &slides)?;

    queue!(w, Show)?;

    disable_raw_mode()?;

    Ok(())
}
