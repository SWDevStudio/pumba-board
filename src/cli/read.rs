//! Reading commands: print the result of `pumba_core::api` as text or `--json`.
use super::{BriefArgs, Common, HashArgs, IdArgs, RulesArgs, fail, open};
use pumba_core::api::{self, ApiErr, BriefOpts};
use serde::Serialize;

/// Print a result: JSON in one line or the rendered text; or the error.
fn emit<T: Serialize>(c: &Common, r: Result<T, ApiErr>, text: impl FnOnce(&T) -> String) -> i32 {
    match r {
        Ok(v) => {
            if c.json {
                println!("{}", api::to_json(&v));
            } else {
                print!("{}", text(&v));
            }
            api::code::OK
        }
        Err(e) => fail(c, &e),
    }
}

pub fn brief(a: BriefArgs) -> i32 {
    let board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    match api::brief(&board, &a.id, &BriefOpts { known: a.known, body: a.body }, api::now_secs()) {
        Ok(b) => {
            // BLOCKED and DELETED still print the full answer (the blockers), then exit non-zero
            if a.c.json {
                println!("{}", api::to_json(&b));
            } else {
                print!("{}", b.render());
            }
            if b.exit != api::code::OK {
                eprintln!("pumba: {} {} ({})", b.id, b.status, b.exit);
            }
            b.exit
        }
        Err(e) => fail(&a.c, &e),
    }
}

pub fn rules(a: RulesArgs) -> i32 {
    let board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    emit(&a.c, api::rules(&board, &a.parts), |r| r.render())
}

pub fn unblocks(a: IdArgs) -> i32 {
    let board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    emit(&a.c, api::unblocks(&board, &a.id), |r| r.render())
}

pub fn column(a: IdArgs) -> i32 {
    let board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    emit(&a.c, api::column(&board, &a.id), |r| r.render())
}

pub fn rules_hash(a: HashArgs) -> i32 {
    let board = match open(&a.c) {
        Ok(b) => b,
        Err(code) => return code,
    };
    emit(&a.c, Ok(api::rules_hash(&board)), |r| r.render(a.parts))
}
