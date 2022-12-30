use crate::config::Config;
use crate::sponsorblock::{self, *};
use crate::utils::get_youtube_id;

use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;
use tokio::select;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct SortedSegments {
    skippable: Segments,
    mutable: Segments,
    poi: Option<Segment>,
    full: Option<Segment>,
}

pub struct Worker {
    sorted_segments: Arc<Mutex<SortedSegments>>,
    rt: Runtime,
    token: CancellationToken,
    join: JoinHandle<()>,
}

impl Worker {
    pub fn new(config: Config, path: String) -> Option<Self> {
        let id = get_youtube_id(path)?; // If not a YT video then do nothing

        let sorted_segments = Arc::new(Mutex::new(SortedSegments::default()));
        let rt = Runtime::new().unwrap();
        let token = CancellationToken::new();
        let join = rt.spawn(Self::run(config, id, sorted_segments.clone(), token.clone()));

        Some(Worker {
            sorted_segments,
            rt,
            token,
            join,
        })
    }

    async fn run(config: Config, id: String, sorted_segments: Arc<Mutex<SortedSegments>>, token: CancellationToken) {
        select! {
            _ = token.cancelled() => {
                log::debug!("Thread cancelled. Segments couldn't be retrieved in time");
            },
            segments = sponsorblock::fetch(config.server_address, config.privacy_api, id, config.categories, config.action_types) => {
                let mut segments = segments.unwrap_or_default();

                // Lock only when segments are found
                let mut sorted_segments = sorted_segments.lock().unwrap();

                // The sgments will be searched multiple times by seconds.
                // It's more efficient to split them before.

                (*sorted_segments).skippable = segments.drain_filter(|s| s.action == Action::Skip).collect();
                log::info!("Found {} skippable segment(s)", (*sorted_segments).skippable.len());

                (*sorted_segments).mutable = segments.drain_filter(|s| s.action == Action::Mute).collect();
                log::info!("Found {} muttable segment(s)", (*sorted_segments).mutable.len());

                (*sorted_segments).poi = segments.drain_filter(|s| s.action == Action::Poi).next();
                log::info!("Highlight {}", if (*sorted_segments).poi.is_some() { "found" } else { "not found" });

                (*sorted_segments).full = segments.drain_filter(|s| s.action == Action::Full).next();
                log::info!("Category {}", if (*sorted_segments).full.is_some() { "found" } else { "not found" });
            }
        };
    }

    pub fn get_skip_segment(&self, time_pos: f64) -> Option<Segment> {
        self.sorted_segments
            .lock()
            .unwrap()
            .skippable
            .iter()
            .find(|s| s.is_in_segment(time_pos))
            .cloned()
    }

    pub fn get_mute_segment(&self, time_pos: f64) -> Option<Segment> {
        self.sorted_segments
            .lock()
            .unwrap()
            .mutable
            .iter()
            .find(|s| s.is_in_segment(time_pos))
            .cloned()
    }

    pub fn get_video_poi(&self) -> Option<f64> {
        self.sorted_segments.lock().unwrap().poi.as_ref().map(|s| s.segment[0])
    }

    pub fn get_video_category(&self) -> Option<Category> {
        self.sorted_segments.lock().unwrap().full.as_ref().map(|s| s.category)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        log::debug!("Stopping worker");
        self.token.cancel();
        self.rt.block_on(&mut self.join).unwrap();
    }
}
