use genanki_rs::*;
use rand::Rng;
use sacrifice::prelude::*;
use sacrifice::{Chess, Position};
use sacrifice::{Color, File, Rank, Role, Square};
use shakmaty::board::Board;
use std::str::FromStr;
use std::{env, fs};

fn build_model() -> Model {
    const MODEL_ID: i64 = 0x25bbbb4805f55380;
    let css = include_str!("../ankitemplate/css.css");
    let front_html = include_str!("../ankitemplate/front.html");
    let back_html = include_str!("../ankitemplate/back.html");
    Model::new_with_options(
        MODEL_ID,
        "Mchess",
        vec![
            Field::new("front_board"),
            Field::new("front_text"),
            Field::new("back_board"),
            Field::new("back_text"),
        ],
        vec![Template::new("Chess card").qfmt(front_html).afmt(back_html)],
        Some(css),
        None,
        None,
        None,
        None,
    )
}

fn board_to_txt(board: &Board, side: Color) -> String {
    let mut utf16 = Vec::<u16>::new();

    utf16.push(0xe300);
    for _ in 0..8 {
        utf16.push(0xe301);
    }
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
    for _ in 0..8 {
        utf16.push(0xe306);
    }
    utf16.push(0xe307);
    String::from_utf16(&utf16).expect("error converting board txt from utf16")
}

fn build_deck(pgn: &str, side: Color) -> Deck {
    let model = build_model();
    let mut rng = rand::thread_rng();
    let mut deck = Deck::new(rng.gen(), "Chess", "chess repertoire deck");
    let start_position = Chess::default();
    let game = sacrifice::read_pgn(pgn);
    let mut stack = vec![game.root()];
    while let Some(node) = stack.pop() {
        if node.board(&start_position).turn() != side {
            if let (Some(prev), Some(mov)) = (node.parent(), node.prev_move()) {
                let back_board = board_to_txt(node.board(&start_position).board(), side);
                let comment = node.comment().unwrap_or_default();
                let front_board = board_to_txt(prev.board(&start_position).board(), side);
                let back_txt = format!("{}<br/>{}", mov, comment);
                deck.add_note(
                    Note::new(
                        model.clone(),
                        vec![&front_board, "", &back_board, &back_txt],
                    )
                    .expect("error building note"),
                );
            }
        }
        for i in node.variation_vec() {
            stack.push(i);
        }
    }
    deck
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "usage: {} pgn_input_filename color apkg_output_filename",
            args[0]
        );
        return;
    }
    let pgn = fs::read_to_string(&args[1]).expect("could not read input pgn file");
    let side =
        Color::from_str(&args[2]).unwrap_or_else(|_| panic!("{} is not a correct color", args[2]));
    let deck = build_deck(&pgn, side);
    let mut package = Package::new(vec![deck], vec!["ankitemplate/_chess_merida_unicode.ttf"])
        .expect("could not build anki package");
    package
        .write_to_file(&args[3])
        .expect("could not write anki package");
}
