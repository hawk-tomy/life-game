use anyhow::{anyhow, ensure, Error, Result};
use chrono::prelude::Local;
use clap::{ArgGroup, Parser};
use crossterm::{
    cursor::{Hide, MoveTo, SetCursorStyle, Show},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind},
    execute, queue,
    terminal::{size, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use once_cell::sync::Lazy;
use rand::{thread_rng, Rng};
use regex::Regex;
use std::{
    cmp::max,
    concat,
    fmt::Display,
    fs::{read_to_string, write},
    io::{stdout, Stdout, Write},
    path::PathBuf,
    str::FromStr,
    thread::sleep,
    time::Duration,
};

#[allow(unused_macros)]
macro_rules! debug {
    ($v:expr) => {{
        println!("{} = {:?}", stringify!($v), $v);
        $v
    }};
    ($msg:literal, $v:expr) => {{
        println!("{}; {} = {:?}", $msg, stringify!($v), $v);
        $v
    }};
}

macro_rules! press {
    (char $c:pat) => {
        Event::Key(KeyEvent {
            code: KeyCode::Char($c),
            kind: KeyEventKind::Press,
            ..
        })
    };
    (enter) => {
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            kind: KeyEventKind::Press,
            ..
        })
    };
    (left) => {
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            kind: KeyEventKind::Press,
            ..
        })
    };
    (right) => {
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            kind: KeyEventKind::Press,
            ..
        })
    };
    (up) => {
        Event::Key(KeyEvent {
            code: KeyCode::Up,
            kind: KeyEventKind::Press,
            ..
        })
    };
    (down) => {
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            kind: KeyEventKind::Press,
            ..
        })
    };
    ($c:pat) => {
        Event::Key(KeyEvent {
            code: $c,
            kind: KeyEventKind::Press,
            ..
        })
    };
}

static POINT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<width>\d+):(?P<height>\d+)$").unwrap());
static FILE_FORMAT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<width>\d+):(?P<height>\d+)\n(?P<data>[01\n]+)$").unwrap());

fn point_from_str(s: &str) -> Result<(u16, u16)> {
    let cap = POINT_REGEX.captures(s).ok_or_else(|| {
        anyhow!(concat!(
            "Invalid Format!.",
            r#"note:: you must use a "<width>:<height>" format."#
        ))
    })?;
    Ok((
        cap.name("width").unwrap().as_str().parse()?,
        cap.name("height").unwrap().as_str().parse()?,
    ))
}

#[derive(Debug, Clone, Copy)]
struct Size {
    width: u16,
    height: u16,
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Default for Size {
    fn default() -> Self {
        Self {
            width: 160,
            height: 32,
        }
    }
}

impl FromStr for Size {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (width, height) = if s == "M" {
            let (w, h) = size()?;
            (
                w,
                h.checked_sub(5).ok_or(anyhow!("Terminal is too small!"))?,
            )
        } else {
            point_from_str(s)?
        };
        Ok(Self { width, height })
    }
}

fn min_15(v: &str) -> Result<u64> {
    let v = v.parse::<u64>()?;
    Ok(max(v, 15))
}

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("initialize").required(false).args(["file", "random"])
))]
struct Args {
    #[arg(
        short,
        long,
        default_value = "160:32",
        default_value_if("max", "true", "M")
    )]
    size: Size,
    #[arg(short = 'M', long, conflicts_with = "size")]
    max: bool,
    #[arg(short, long)]
    random: bool,
    #[arg(short, long, value_name = "FILE", conflicts_with_all = ["size", "random", "max"])]
    file: Option<PathBuf>,
    #[arg(short, long, default_value = "100", help = "ms. min: 15ms.", value_parser = min_15)]
    duration: u64,
}

impl Args {
    pub(crate) fn init_from_file(&self, path: PathBuf) -> Result<Vec<bool>> {
        let path = path.as_path();

        ensure!(path.exists() && path.is_file());

        let str = read_to_string(path)?;
        let cap = FILE_FORMAT_REGEX
            .captures(str.as_str())
            .ok_or_else(|| anyhow!("Invalid File Format!"))?;

        let width = cap.name("width").unwrap().as_str().parse::<u16>()?;
        let height = cap.name("height").unwrap().as_str().parse::<u16>()?;
        let length = width as usize * height as usize;

        let data = cap.name("data").unwrap().as_str();
        let mut game: Vec<bool> = Vec::with_capacity(length);
        for c in data.chars() {
            match c {
                '\n' => continue,
                '0' => game.push(false),
                '1' => game.push(true),
                _ => panic!("unreachable"),
            }
        }

        ensure!(length == game.len(), "Invalid Data!");

        Ok(game)
    }
}

#[derive(Debug)]
struct State {
    size: Size,
    time: usize,
    duration: u64,
    len: usize,
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}, {}times", self.size, self.time)
    }
}

impl State {
    fn new(args: &Args) -> Result<Self> {
        Ok(Self {
            size: args.size,
            time: 0,
            duration: args.duration,
            len: usize::checked_mul(args.size.width.into(), args.size.height.into())
                .ok_or_else(|| anyhow!("overflow"))?,
        })
    }

    fn move_to(&self, pos: (u16, u16), amount: (i16, i16)) -> Result<(u16, u16)> {
        let (px, py) = pos;
        let (ax, ay) = amount;
        Ok((
            ((self.size.width as i32 + px as i32 + ax as i32) % self.size.width as i32) as u16,
            ((self.size.height as i32 + py as i32 + ay as i32) % self.size.height as i32) as u16,
        ))
    }
}

#[derive(Debug)]
struct Game {
    game: Vec<bool>,
    state: State,
}

impl Display for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}\n{}", self.show_board(), self.state)
    }
}

impl Game {
    fn init(args: &Args) -> Result<Self> {
        let state = State::new(args)?;

        let game = if let Some(path) = args.file.clone() {
            args.init_from_file(path)?
        } else if args.random {
            let mut base = vec![false; state.len];
            thread_rng().fill(&mut base[..]);
            base
        } else {
            vec![false; state.len]
        };

        Ok(Self { game, state })
    }

    fn show_board(&self) -> String {
        let chars: Vec<char> = self
            .game
            .iter()
            .map(|&v| if v { '@' } else { '-' })
            .collect();
        let mut formatted = String::with_capacity(
            usize::checked_add(self.state.len, self.state.size.height.into())
                .ok_or_else(|| anyhow!("overflow"))
                .unwrap(),
        );

        let mut i = 0;
        for _ in 0..self.state.size.height {
            for _ in 0..self.state.size.width {
                formatted.push(chars[i]);
                i += 1;
            }
            formatted.push('\n');
        }
        formatted
    }

    fn next(&mut self) -> Result<()> {
        self.state.time += 1;
        self.game = self
            .game
            .iter()
            .enumerate()
            .map(|(i, &v)| -> Result<bool> {
                let pts = self.get_pt(i)?;
                let alive = pts.iter().filter(|&&j| self.game[j]).count();
                Ok(if v {
                    // idx: alive
                    1 < alive && alive < 4
                } else {
                    // idx: dead
                    alive == 3
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    fn get_pt(&self, idx: usize) -> Result<[usize; 8]> {
        // cu ru rm rd cd ld lm lu
        let idx: u32 = idx.try_into()?;

        let width = self.state.size.width.into();
        let size: u32 = self.state.len.try_into()?;

        let left_weight = if idx % width == 0 { width } else { 0 };
        let right_weight = if idx % width == width - 1 { width } else { 0 };

        let cu = (size + idx - width) % size;
        let cd = (size + idx + width) % size;

        let ru = (size + idx + 1 - right_weight - width) % size;
        let rm = (size + idx + 1 - right_weight) % size;
        let rd = (size + idx + 1 - right_weight + width) % size;

        let lu = (size + idx - 1 + left_weight - width) % size;
        let lm = (size + idx - 1 + left_weight) % size;
        let ld = (size + idx - 1 + left_weight + width) % size;

        Ok([cu, ru, rm, rd, cd, ld, lm, lu].map(|v| v as usize))
    }

    fn check_pos(&self, pos: (u16, u16)) -> Result<()> {
        let (x, y) = pos;
        ensure!(
            x < self.state.size.width,
            "x:{} is bigger than width:{}",
            x,
            self.state.size.width
        );
        ensure!(
            y < self.state.size.height,
            "y:{} is bigger than height:{}",
            y,
            self.state.size.height
        );
        Ok(())
    }

    fn set_pos(&mut self, pos: (u16, u16)) -> Result<()> {
        self.check_pos(pos)?;
        let (x, y) = pos;
        let idx = (y * self.state.size.width + x) as usize;
        self.game[idx] = !self.game[idx];
        Ok(())
    }

    fn move_to(&self, pos: (u16, u16), amount: (i16, i16)) -> Result<(u16, u16)> {
        self.state.move_to(pos, amount)
    }

    fn save(&self) -> Result<String> {
        let path = Local::now().format("./%F_%H.%M.%ST%z.txt").to_string();
        let mut data = format!("{}:{}", self.state.size.width, self.state.size.height);
        for (i, &v) in self.game.iter().enumerate() {
            if i % self.state.size.width as usize == 0 {
                data.push('\n')
            }
            data.push(if v { '1' } else { '0' });
        }
        write(&path, data)?;

        Ok(format!("success save to {}", path))
    }
}

fn main() -> Result<()> {
    // setup App by cmd line options
    let args = Args::parse();
    let mut game = Game::init(&args)?;
    // setup tui
    let mut stdout = stdout();
    execute!(stdout, Hide, EnterAlternateScreen)?;
    // run app logic. error logic is after.
    let result = main_loop(&mut stdout, &mut game);
    // clean up
    execute!(
        stdout,
        MoveTo(0, 0),
        Clear(ClearType::FromCursorDown),
        Show,
        LeaveAlternateScreen
    )?;
    stdout.flush()?;

    result
}

fn main_loop(stdout: &mut Stdout, game: &mut Game) -> Result<()> {
    let mut info: Option<String> = None;
    loop {
        queue!(stdout, MoveTo(0, 0), Clear(ClearType::FromCursorDown))?;
        println!(
            "{}\n<q>: quit program.\t<a>: auto run.\t<e>: switch to editor.\t<s>: save to file.\t<CR>: next.\n{}",
            game, if let Some(msg) = info { msg } else { "".to_string() }
        );
        info = None;
        match read()? {
            press!(char 'q') => break,
            press!(enter) => game.next()?,
            press!(char 'e') => {
                execute!(stdout, Show, SetCursorStyle::BlinkingBlock)?;
                editor_loop(stdout, game)?;
                execute!(stdout, Show, SetCursorStyle::DefaultUserShape)?;
            }
            press!(char 'a') => auto_loop(stdout, game)?,
            press!(char 's') => info = Some(game.save()?),
            _ => continue,
        };
    }
    Ok(())
}

fn auto_loop(stdout: &mut Stdout, game: &mut Game) -> Result<()> {
    let dur = Duration::from_millis(game.state.duration);
    let zero_sec = Duration::from_secs(0);

    loop {
        game.next()?;
        queue!(stdout, MoveTo(0, 0), Clear(ClearType::FromCursorDown))?;
        println!(
            "{}\n<q>: quit auto run.\tduration: {}ms",
            game, game.state.duration
        );

        sleep(dur);
        if poll(zero_sec)? {
            match read()? {
                press!(char 'q') => break,
                _ => continue,
            }
        }
    }

    Ok(())
}

fn editor_loop(stdout: &mut Stdout, game: &mut Game) -> Result<()> {
    let mut pos = (0u16, 0u16);
    loop {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::FromCursorDown))?;
        println!(
            "{}\n`<h>`:left\t`<j>`:down\t`<k>`:up\t`<l>`:right\t`<CR>`: reverse.\t`q`: quit editor mode.\n",
            game
        );
        execute!(stdout, MoveTo(pos.0, pos.1))?;
        pos = match read()? {
            press!(char 'q') => break,
            press!(char 'h') | press!(left) => game.move_to(pos, (-1, 0)).unwrap_or(pos),
            press!(char 'j') | press!(down) => game.move_to(pos, (0, 1)).unwrap_or(pos),
            press!(char 'k') | press!(up) => game.move_to(pos, (0, -1)).unwrap_or(pos),
            press!(char 'l') | press!(right) => game.move_to(pos, (1, 0)).unwrap_or(pos),
            press!(enter) => {
                game.set_pos(pos)?;
                continue;
            }
            _ => continue,
        };
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_of_test() -> Result<()> {
        let args = Args {
            size: Size::default(),
            max: false,
            file: None,
            duration: 100,
            random: false,
        };
        let mut game = Game::init(&args)?;
        println!("{}", game);
        game.set_pos((15, 20))?;
        println!("{}", game);
        Ok(())
    }

    macro_rules! inner {
        ( $v:ident) => {};
        ( $v:ident, $e:literal) => {{$v.push($e != 0);}};
        ( $v:ident, $e:literal, $($a:literal),* ) => {{
            $v.push($e != 0);
            inner!($v, $($a),* );
        }};
    }

    macro_rules! board_init {
        ($($e:literal),*) => {{
            let mut v = Vec::new();
            inner!(v, $($e),*);
            v
        }};
    }

    #[test]
    fn blinker_test() -> Result<()> {
        let args = Args {
            size: Size {
                width: 5,
                height: 5,
            },
            max: false,
            file: None,
            duration: 100,
            random: false,
        };
        let mut game = Game::init(&args)?;
        game.set_pos((1, 2))?;
        game.set_pos((2, 2))?;
        game.set_pos((3, 2))?;
        println!("{}", game);
        game.next()?;
        println!("{}", game);
        game.next()?;
        println!("{}", game);
        assert_eq!(
            game.game,
            board_init!(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
        );
        Ok(())
    }

    #[test]
    fn get_pt() -> Result<()> {
        let args = Args {
            size: Size {
                width: 3,
                height: 3,
            },
            max: false,
            file: None,
            duration: 100,
            random: false,
        };
        let game = Game::init(&args)?;
        // 0 1 2 0 1 2
        // 3 4 5 3 4 5
        // 6 7 8 6 7 8
        // 0 1 2 0 1 2
        // 3 4 5 3 4 5
        // 6 7 8 6 7 8

        //                           cu ru rm rd cd ld lm lu
        assert_eq!(game.get_pt(4)?, [1, 2, 5, 8, 7, 6, 3, 0,]);
        assert_eq!(game.get_pt(0)?, [6, 7, 1, 4, 3, 5, 2, 8,]);
        assert_eq!(game.get_pt(1)?, [7, 8, 2, 5, 4, 3, 0, 6,]);
        assert_eq!(game.get_pt(2)?, [8, 6, 0, 3, 5, 4, 1, 7,]);
        assert_eq!(game.get_pt(3)?, [0, 1, 4, 7, 6, 8, 5, 2,]);
        assert_eq!(game.get_pt(4)?, [1, 2, 5, 8, 7, 6, 3, 0,]);
        assert_eq!(game.get_pt(5)?, [2, 0, 3, 6, 8, 7, 4, 1,]);
        assert_eq!(game.get_pt(6)?, [3, 4, 7, 1, 0, 2, 8, 5,]);
        assert_eq!(game.get_pt(7)?, [4, 5, 8, 2, 1, 0, 6, 3,]);
        assert_eq!(game.get_pt(8)?, [5, 3, 6, 0, 2, 1, 7, 4,]);

        Ok(())
    }
}
