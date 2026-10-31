//! Writing commands. The write itself (fresh read, check, atomic commit, AGENTS.md) is in
//! `pumba_core::api`; here the board is opened, the result printed and the exit code chosen.
use super::{Common, MoveArgs, NoteArgs, fail, open};
use pumba_core::api::{self, MoveReq, NoteReq};

fn print<T: serde::Serialize>(c: &Common, v: &T, text: String) {
    if c.json {
        println!("{}", api::to_json(v));
    } else {
        print!("{text}");
    }
}

pub fn move_cmd(a: MoveArgs) -> i32 {
    let mut board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let req = MoveReq { id: a.id, to: a.to, expect_from: a.expect_from, note: a.note, user_approved: a.user_approved, at: a.at };
    match api::move_card(&mut board, &req, api::now_secs()) {
        Ok(o) => {
            print(&a.c, &o, o.render());
            api::code::OK
        }
        Err(e) => fail(&a.c, &e),
    }
}

pub fn note_cmd(a: NoteArgs) -> i32 {
    let mut board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let req = NoteReq { id: a.id, text: a.text, user_approved: a.user_approved, at: a.at };
    match api::add_note(&mut board, &req, api::now_secs()) {
        Ok(o) => {
            print(&a.c, &o, o.render());
            api::code::OK
        }
        Err(e) => fail(&a.c, &e),
    }
}
