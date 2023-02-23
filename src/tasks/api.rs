use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use flume::Sender;
use once_cell::sync::Lazy;
use tokio::task::JoinSet;
use ytpapi2::{YoutubeMusicInstance, YoutubeMusicPlaylistRef};

use crate::{
    run_service,
    structures::performance,
    systems::logger::log_,
    term::{ManagerMessage, Screens},
};

const TEXT_COOKIES_EXPIRED_OR_INVALID: &str =
    "The `headers.txt` file is not configured correctly. \nThe cookies are expired or invalid.";

pub fn spawn_api_task(updater_s: Arc<Sender<ManagerMessage>>) {
    run_service(async move {
        log_("API task on");
        let guard = performance::guard("API task");
        let client = YoutubeMusicInstance::from_header_file(
            PathBuf::from_str("headers.txt").unwrap().as_path(),
        )
        .await;
        match client {
            Ok(api) => {
                let api = Arc::new(api);
                let mut set = JoinSet::new();
                let api_ = api.clone();
                let updater_s_ = updater_s.clone();
                set.spawn(async move {
                    let search_results = api_.get_home(2).await;
                    match search_results {
                        Ok(e) => {
                            for playlist in e.playlists {
                                spawn_browse_playlist_task(
                                    playlist.clone(),
                                    api_.clone(),
                                    updater_s_.clone(),
                                )
                            }
                        }
                        Err(_) => todo!(),
                    }
                });
                let api_ = api.clone();
                let updater_s_ = updater_s.clone();
                set.spawn(async move {
                    let search_results = api_.get_library(2).await;
                    match search_results {
                        Ok(e) => {
                            for playlist in e {
                                spawn_browse_playlist_task(
                                    playlist.clone(),
                                    api_.clone(),
                                    updater_s_.clone(),
                                )
                            }
                        }
                        Err(e) => {
                            log_(format!("{e:?}"));
                        }
                    }
                });
                while let Some(e) = set.join_next().await {
                    e.unwrap();
                }
            }
            Err(e) => match &e {
                ytpapi2::YoutubeMusicError::NoCookieAttribute
                | ytpapi2::YoutubeMusicError::NoSapsidInCookie
                | ytpapi2::YoutubeMusicError::InvalidCookie
                | ytpapi2::YoutubeMusicError::NeedToLogin
                | ytpapi2::YoutubeMusicError::CantFindInnerTubeApiKey(_)
                | ytpapi2::YoutubeMusicError::CantFindInnerTubeClientVersion(_)
                | ytpapi2::YoutubeMusicError::CantFindVisitorData(_)
                | ytpapi2::YoutubeMusicError::IoError(_) => {
                    log_(TEXT_COOKIES_EXPIRED_OR_INVALID);
                    log_(format!("{e:?}"));
                    updater_s
                        .send(
                            ManagerMessage::Error(
                                TEXT_COOKIES_EXPIRED_OR_INVALID.to_string(),
                                Box::new(Some(ManagerMessage::Quit)),
                            )
                            .pass_to(Screens::DeviceLost),
                        )
                        .unwrap();
                }
                e => {
                    log_(format!("{e:?}"));
                }
            },
        }
        drop(guard);
    });
}

static BROWSED_PLAYLISTS: Lazy<Mutex<Vec<(String, String)>>> = Lazy::new(|| Mutex::new(vec![]));

fn spawn_browse_playlist_task(
    playlist: YoutubeMusicPlaylistRef,
    api: Arc<YoutubeMusicInstance>,
    updater_s: Arc<Sender<ManagerMessage>>,
) {
    {
        let mut k = BROWSED_PLAYLISTS.lock().unwrap();
        if k.iter()
            .any(|(name, id)| name == &playlist.name && id == &playlist.browse_id)
        {
            return;
        }
        k.push((playlist.name.clone(), playlist.browse_id.clone()));
    }

    run_service(async move {
        let guard = format!("Browse playlist {}", playlist.name);
        let guard = performance::guard(&guard);
        match api.get_playlist(&playlist, 5).await {
            Ok(videos) => {
                let _ = updater_s
                    .send(
                        ManagerMessage::AddElementToChooser((
                            format!("{} ({})", playlist.name, playlist.subtitle),
                            videos,
                        ))
                        .pass_to(Screens::Playlist),
                    );
            }
            Err(e) => {
                log_(format!("{e:?}"));
            }
        }

        drop(guard);
    });
}
