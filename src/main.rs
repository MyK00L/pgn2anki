use clap::Parser;
use genanki_rs::*;
use rand::Rng;
use sacrifice::prelude::*;
use sacrifice::{Chess, Position};
use sacrifice::{Color, File, Move, Rank, Role, Square};
use shakmaty::board::Board;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use std::fs;

fn build_model() -> Model {
    const MODEL_ID: i64 = 0x25bbbb4805f55380;
    let css = include_str!("../ankitemplate/css.css");
    Model::new_with_options(
        MODEL_ID,
        "Mchess",
        vec![Field::new("front"), Field::new("back")],
        vec![Template::new("Chess card")
            .qfmt("{{front}}")
            .afmt("{{back}}")],
        Some(css),
        None,
        None,
        None,
        None,
    )
}

fn build_squares_model() -> Model {
    const MODEL_ID: i64 = 0x25bbbb4805f55381;
    let css = include_str!("../ankitemplate/css.css");
    Model::new_with_options(
        MODEL_ID,
        "Mchess",
        vec![Field::new("square_name"), Field::new("square_board"), Field::new("square_color")],
        vec![Template::new("Chess card").qfmt("Where on the board is {{square_name}}?").afmt("<div class='chess'>{{square_board}}</div><br/>{{square_name}}"),
        Template::new("Chess card").qfmt("What square is this?<br/><div class='chess'>{{square_board}}</div>").afmt("{{square_name}}<br/><div class='chess'>{{square_board}}</div>"),
        Template::new("Chess card").qfmt("What color is {{square_name}}").afmt("{{square_name}} is {{square_color}}<br/><div class='chess'>{{square_board}}</div>")],
        Some(css),
        None,
        None,
        None,
        None,
    )
}

fn board_to_txt(board: &Board, side: Color, bullets: &[Square]) -> String {
    let mut utf16 = Vec::<u16>::new();

    utf16.push(0xe300);
    utf16.resize(utf16.len() + 8, 0xe301);
    utf16.push(0xe302);
    // <br/>
    utf16.push(0x3c);
    utf16.push(0x62);
    utf16.push(0x72);
    utf16.push(0x2f);
    utf16.push(0x3e);

    let rows: Vec<Rank> = if side == Color::White {
        Rank::ALL.into_iter().rev().collect()
    } else {
        Rank::ALL.into_iter().collect()
    };
    let cols: Vec<File> = if side == Color::White {
        File::ALL.into_iter().collect()
    } else {
        File::ALL.into_iter().rev().collect()
    };

    for row in &rows {
        utf16.push(0xe303);
        for col in &cols {
            let square = Square::from_coords(*col, *row);
            let val: u16 = if let Some(piece) = board.piece_at(square) {
                (match piece.role {
                    Role::King => 0,
                    Role::Queen => 1,
                    Role::Rook => 2,
                    Role::Bishop => 3,
                    Role::Knight => 4,
                    Role::Pawn => 5,
                }) + if piece.color == Color::Black { 6 } else { 0 }
                    + if square.is_dark() { 0xe154 } else { 0x2654 }
            } else if bullets.contains(&square) {
                if square.is_dark() {
                    0xe122
                } else {
                    0x2022
                }
            } else if square.is_dark() {
                0xe100
            } else {
                0x00a0
            };
            utf16.push(val);
        }
        utf16.push(0xe304);
        // <br/>
        utf16.push(0x3c);
        utf16.push(0x62);
        utf16.push(0x72);
        utf16.push(0x2f);
        utf16.push(0x3e);
    }
    utf16.push(0xe305);
    utf16.resize(utf16.len() + 8, 0xe306);
    utf16.push(0xe307);
    String::from_utf16(&utf16).expect("error converting board txt from utf16")
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct DeckMapKey {
    board: Board,
    side: Color,
}
#[derive(Clone, Debug, Default)]
struct DeckMapVal {
    answers: HashMap<Board, HashMap<Move, Vec<String>>>,
    questions: Vec<String>,
}

fn add_to_map(map: &mut HashMap<DeckMapKey, DeckMapVal>, pgn: &str, side: Color) {
    let start_position = Chess::default();
    let game = sacrifice::read_pgn(pgn);
    let mut stack = vec![game.root()];
    while let Some(node) = stack.pop() {
        if node.board(&start_position).turn() != side {
            if let (Some(prev), Some(mov)) = (node.parent(), node.prev_move()) {
                let chess0 = prev.board(&start_position);
                let board0 = chess0.board();
                let chess1 = node.board(&start_position);
                let board1 = chess1.board();

                let key = DeckMapKey {
                    board: board0.clone(),
                    side,
                };

                if !map.contains_key(&key) {
                    map.insert(key.clone(), DeckMapVal::default());
                }

                let val = map.get_mut(&key).unwrap();

                if !val.answers.contains_key(board1) {
                    val.answers.insert(board1.clone(), HashMap::new());
                }

                let answer_val = val.answers.get_mut(board1).unwrap();

                if !answer_val.contains_key(&mov) {
                    answer_val.insert(mov.clone(), Vec::new());
                }

                if let Some(comment) = node.comment() {
                    answer_val.get_mut(&mov).unwrap().push(comment);
                }
                // val.questions.push(stuff)
            }
        }
        for i in node.variation_vec() {
            stack.push(i);
        }
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
fn add_map_to_deck(deck: &mut Deck, map: &mut HashMap<DeckMapKey, DeckMapVal>) {
    let model = build_model();
    for (k, mut v) in map.drain() {
        let questions_txt = v.questions.join("<br/>\n");
        let front = format!(
            r#"<figure>
                <div class="chess">
                {}
                </div>
                <figcaption>
                {}
                </figcaption>
                </figure>"#,
            board_to_txt(&k.board, k.side, &[]),
            questions_txt
        );
        let back = v
            .answers
            .drain()
            .map(|(aboard, av)| {
                let txt = av
                    .iter()
                    .map(|(mov, comments)| format!("{}<br/>{}", mov, comments.join("<br/>")))
                    .fold(String::new(), |a, b| a + &b + "<br/>\n");
                format!(
                    r#"<figure>
                        <div class="chess">
                        {}
                        </div>
                        <figcaption>
                        {}
                        </figcaption>
                        </figure>"#,
                    board_to_txt(&aboard, k.side, &[]),
                    txt
                )
            })
            .fold(String::new(), |a, b| a + &b + "<hr/>\n");
        deck.add_note(
            Note::new_with_options(
                model.clone(),
                vec![&front, &back],
                None,
                Some(vec![&format!("chess::{}", k.side)]),
                Some(&calculate_hash(&k).to_string()),
            )
            .expect("error building note"),
        );
    }
}
fn add_squares_to_deck(deck: &mut Deck) {
    let empty_board = Board::empty();
    let model = build_squares_model(); // name board color
    for square in Square::ALL {
        let note = Note::new_with_options(
            model.clone(),
            vec![
                &square.to_string(),
                &board_to_txt(&empty_board, Color::White, &[square]),
                if square.is_light() { "light" } else { "dark" },
            ],
            None,
            Some(vec![&String::from("chess::square")]),
            Some(&format!("chess_square_{}", square)),
        )
        .expect("could not build square note");
        deck.add_note(note);
    }
}

fn gen_deck(white: &[String], black: &[String], squares: bool) -> Deck {
    let mut map = HashMap::<DeckMapKey, DeckMapVal>::new();
    for filename in white {
        let pgn = fs::read_to_string(filename)
            .unwrap_or_else(|_| panic!("could not read input pgn file {}", filename));
        add_to_map(&mut map, &pgn, Color::White);
    }
    for filename in black {
        let pgn = fs::read_to_string(filename)
            .unwrap_or_else(|_| panic!("could not read input pgn file {}", filename));
        add_to_map(&mut map, &pgn, Color::Black);
    }
    let mut rng = rand::thread_rng();
    let mut deck = Deck::new(rng.gen(), "Chess", "chess repertoire deck");
    if squares {
        add_squares_to_deck(&mut deck);
    }
    add_map_to_deck(&mut deck, &mut map);
    deck
}

/// Convert pgn files into an anki deck
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// filename of the output apkg deck file
    #[arg(required = true)]
    out: String,
    /// wether to add square notes
    #[arg(long)]
    squares: bool,
    /// filenames of pgns to be processed from white's perspective
    #[arg(short)]
    white: Vec<String>,
    /// filenames of pgns to be  processed from black's perspective
    #[arg(short)]
    black: Vec<String>,
}

fn main() {
    let args = Args::parse();
    let deck = gen_deck(&args.white, &args.black, args.squares);
    let mut package = Package::new(vec![deck], vec!["ankitemplate/_chess_merida_unicode.ttf"])
        .expect("could not build anki package");
    package
        .write_to_file(&args.out)
        .expect("could not write anki package");
}
