#![feature(try_blocks)]

use rustube::Error;
use terminal::AppMessage;

use std::{path::PathBuf, str::FromStr, sync::Arc};
use systems::player::player_system;
use systems::{download::downloader, logger::log};

use ytpapi::{Video, YTApi};

mod systems;
mod terminal;

#[derive(Debug, Clone)]
pub enum SoundAction {
    PlayPause,
    Plus,
    Minus,
    Previous,
    Forward,
    Backward,
    Next,
    PlayVideo(Video),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    std::fs::create_dir_all("data/downloads").unwrap();
    if !PathBuf::from_str("headers.txt").unwrap().exists() {
        println!("The `headers.txt` file is not present in the root directory.");
        println!("This file should contain your headers separated by `: `.");
        return Ok(());
    }
    let (updater_s, updater_r) = flume::unbounded::<AppMessage>();
    tokio::task::spawn(async {
        clean();
    });
    let updater_s = Arc::new(updater_s);
    let sa = Arc::new(player_system(updater_s.clone()));
    let sender = downloader(sa.clone());
    {
        let updater_s = updater_s.clone();
        tokio::task::spawn(async move {
            match YTApi::from_header_file(PathBuf::from_str("headers.txt").unwrap().as_path()).await
            {
                Ok(api) => {
                    let api = Arc::new(api);
                    for playlist in api.playlists() {
                        let updater_s = updater_s.clone();
                        let playlist = playlist.clone();
                        let api = api.clone();
                        tokio::task::spawn(async move {
                            match api.browse_playlist(&playlist.browse_id).await {
                                Ok(videos) => {
                                    updater_s
                                        .send(AppMessage::AddElementToChooser((
                                            format!("{} ({})", playlist.name, playlist.subtitle),
                                            videos,
                                        )))
                                        .unwrap();
                                }
                                Err(e) => {
                                    log(format!("{:?}", e));
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    log(format!("{:?}", e));
                }
            }
        });
    }
    {
        let updater_s = updater_s.clone();
        tokio::task::spawn(async move {
            let mut videos = Vec::new();
            for files in std::fs::read_dir("data/downloads").unwrap() {
                let path = files.unwrap().path();
                if path.as_os_str().to_string_lossy().ends_with(".json") {
                    let video =
                        serde_json::from_str(std::fs::read_to_string(path).unwrap().as_str())
                            .unwrap();
                    videos.push(video);
                }
            }
            updater_s
                .send(AppMessage::AddElementToChooser((
                    "Local musics".to_owned(),
                    videos,
                )))
                .unwrap();
        });
    }
    terminal::main(sender.clone(), updater_r, sa).unwrap();
    Ok(())
}

fn clean() {
    for i in std::fs::read_dir("data/downloads").unwrap() {
        let path = i.unwrap().path();
        if path.ends_with(".mp4") {
            let mut path1 = path.clone();
            path1.set_extension("json");
            if !path1.exists() {
                std::fs::remove_file(&path).unwrap();
            }
        }
    }
}