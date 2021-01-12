use crossterm::{
    cursor::{Hide, MoveTo, Show, self},
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
use clap::{Arg, App};
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

fn text_size(s: &str) -> (usize,usize) {
    let w = 1 + s.lines().fold(0, |acc, l| acc.max(l.len()));

    (w,s.lines().count())
}

impl FileTypes {
    fn action(&self, _w: &mut impl Write) -> Result<()> {
        match self {
            FileTypes::Open(path) | FileTypes::Image(path) => {
                #[cfg(target_os = "linux")]
                Command::new("xdg-open").arg(path).output()?;
                #[cfg(target_os = "macos")]
                Command::new("open")
                    .arg("-W")
                    .arg("-F")
                    .arg("-n")
                    .arg(path)
                    .output()?;
            }
            FileTypes::GifAnimation(path) => {
                disable_raw_mode()?;
                Command::new("viu").arg("-1").arg(path).status()?;
                enable_raw_mode()?;
            }
            _ => (),
        }

        Ok(())
    }

    fn write_text(w: &mut impl Write, txt:&String) -> Result<()> {
        let (width, height) = terminal::size().unwrap();
        let top = height.saturating_sub(txt.lines().count() as u16)  /2;
        
        for (idx,l) in txt.lines().enumerate() {
            let x = width.saturating_sub(l.len() as u16)  /2;
            w.queue(cursor::MoveTo(x,top + idx as u16))?;
            w.write_all(l.as_bytes())?;
        }
        
        w.flush()?;

        Ok(())
    }

    fn show(&self, w: &mut impl Write, margin: usize) -> Result<()> {
        match self {
            FileTypes::GifAnimation(path) => {
                disable_raw_mode()?;
                Command::new("viu").arg("-s").arg(path).status()?;
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
                Self::write_text(w,txt)?;
            }
            FileTypes::Markdown(path) => {
                let (width, height) = terminal::size().unwrap();
                let markdown = fs::read_to_string(Path::new(path))?;
                let (text_w,_) = text_size(markdown.as_str());

                let area_w = text_w.min(width as usize- (margin*2)) as u16;
                let area_h = height - (margin as u16 * 2);

                let x = 0.max((width - area_w) / 2);
                let y = 0.max((height - area_h) / 2);

                MadSkin::default()
                    .write_in_area(&markdown, &Area::new(x, y, area_w, area_h))
                    .unwrap();
            }
            FileTypes::Open(path) => {
                let txt = format!("External file:\n{}\n\npress enter to open",path);
                Self::write_text(w, &txt)?;
            }
            FileTypes::Code(path) => {
                let (width, height) = terminal::size().unwrap();
                let content = fs::read_to_string(Path::new(path))?;
                let text_size = text_size(content.as_str()); 
                let x = (width - text_size.0 as u16)/2;
                let y = (height - text_size.1 as u16)/2;

                // Load these once at the start of your program
                let ps = SyntaxSet::load_defaults_newlines();
                let ts = ThemeSet::load_defaults();

                let syntax = ps.find_syntax_by_extension("rs").unwrap();
                let mut highlighter = HighlightLines::new(syntax, &ts.themes["Solarized (light)"]);

                for (idx,line) in LinesWithEndings::from(content.as_str()).enumerate() {
                    let ranges: Vec<(Style, &str)> = highlighter.highlight(line, &ps);
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    w.queue(cursor::MoveTo(x,y+idx as u16))?;
                    w.write_all(escaped.to_string().as_bytes())?;
                }

                w.queue(cursor::MoveTo(0,0))?;
                w.flush()?;
            }
            FileTypes::FIGlet(txt) => {
                //TODO: draw manually to allow centering
                let standard_font = FIGfont::standand().unwrap();
                let figure = standard_font.convert(txt).unwrap();
                disable_raw_mode()?;
                w.write_all(figure.to_string().as_bytes())?;
                enable_raw_mode()?;
            }
        }

        Ok(())
    }
}

fn present(w: &mut impl Write, slides: &Slides) -> Result<()> {
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
            Input::Action => {
                if let Some(file) = slides.files.get(idx) {
                    file.action(w)?;
                }
            },
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
    Action,
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
            KeyEvent {
                code: KeyCode::Enter, ..
            } => Ok(Input::Action),

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
    let matches = App::new("presentui")
        .version("0.1.0")
        .author("Stephan D. <presentui@extrawurst.org>")
        .about("terminal presenting")
        .arg(Arg::with_name("file")
                 .short("f")
                 .long("file")
                 .takes_value(true)
                 .required(true)
                 .help("input file (*.ron)"))
        .get_matches();

    let f = File::open(matches.value_of("file").unwrap()).expect("Failed opening file");
    let slides: Slides = from_reader(f).expect("Failed to parse ron");

    let mut w = stderr();

    enable_raw_mode()?;
    queue!(w, Hide)?;

    present(&mut w, &slides)?;

    queue!(w, Show)?;
    disable_raw_mode()?;

    Ok(())
}
