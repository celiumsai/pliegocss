use fs2::FileExt;
use std::{
    env,
    fs::OpenOptions,
    io::{self, Read, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("provide at least one publication lock path".into());
    }
    paths.sort();
    paths.dedup();

    let mut locks = Vec::with_capacity(paths.len());
    for path in paths {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.lock_exclusive()?;
        locks.push(file);
    }

    println!("ready");
    io::stdout().flush()?;
    let _ = io::stdin().read(&mut [0_u8; 1]);

    for file in locks.iter().rev() {
        FileExt::unlock(file)?;
    }
    Ok(())
}
