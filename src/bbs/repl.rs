use anyhow::Result;
use reedline::{DefaultPrompt, Reedline, Signal};

use crate::bbs::{
    service::BBS,
    storage::{Storage, UserPkHash},
};

pub async fn run_repl() -> Result<()> {
    let mut line_editor = Reedline::create();
    let storage = Storage::memory();

    let prompt = DefaultPrompt::default();

    let mut bbs = BBS::new(storage);
    bbs.init().await?;

    loop {
        let sig = line_editor.read_line(&prompt);
        match sig {
            Ok(Signal::Success(buffer)) => match bbs.handle([0u8; 32], "Nobody", &buffer).await {
                Ok(out) => {
                    for line in out {
                        println!("{}", line);
                    }
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                }
            },
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
                println!("\nAborted!");
                break;
            }
            x => {
                println!("Event: {:?}", x);
            }
        }
    }
    Ok(())
}
