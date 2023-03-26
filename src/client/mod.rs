mod config;
mod worker;

use config::Config;
use worker::Worker;

use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use mpv_client::{mpv_handle, osd, ClientMessage, Event, Format, Handle, Property, Result};
use sponsorblock_client::Segment;

static NAME_PROP_PATH: &str = "path";
static NAME_PROP_TIME: &str = "time-pos";
static NAME_PROP_MUTE: &str = "mute";

const REPL_PROP_TIME: u64 = 1;
const REPL_PROP_MUTE: u64 = 2;

macro_rules! osd_info {
    ($client:expr, $duration:expr, $($arg:tt)*) => {
        log::info!($($arg)*);
        if $client.config.skip_notice {
            let _ = osd!($client, $duration, $($arg)*);
        }
    };
}

pub struct Client {
    handle: *mut mpv_handle,
    config: Config,
    worker: Worker,
    mute_segment: Option<Segment>,
    mute_sponsorblock: bool,
}

impl Client {
    pub fn from_ptr(handle: *mut mpv_handle) -> Self {
        Self {
            handle,
            config: Config::get(),
            worker: Worker::default(),
            mute_segment: None,
            mute_sponsorblock: false,
        }
    }

    pub fn exec(&mut self) -> Result<()> {
        loop {
            // Wait for MPV events indefinitely
            match self.wait_event(-1.) {
                Event::StartFile(_data) => self.start_file()?,
                Event::PropertyChange(REPL_PROP_TIME, data) => self.time_change(data)?,
                Event::PropertyChange(REPL_PROP_MUTE, data) => self.mute_change(data),
                Event::ClientMessage(data) => self.client_message(data)?,
                Event::EndFile(_data) => self.end_file()?,
                Event::Shutdown => return Ok(()),
                _ => {}
            };
        }
    }

    fn get_youtube_id<'b>(&self, path: &'b str) -> Option<&'b str> {
        let capture = self.config.youtube_regex.captures(&path.as_ref())?;
        capture.get(1).map(|m| m.as_str())
    }

    fn start_file(&mut self) -> Result<()> {
        log::trace!("Received start-file event");

        let path: String = self.get_property(NAME_PROP_PATH)?;
        if let Some(id) = self.get_youtube_id(&path) {
            let parent = self.name().into();
            let child = format!("{}-worker", parent);
            let client = self.create_client(child)?;

            self.worker.start(client, parent, self.config.clone(), id.into());
            self.observe_property(REPL_PROP_TIME, NAME_PROP_TIME, f64::MPV_FORMAT)?;
            self.observe_property(REPL_PROP_MUTE, NAME_PROP_MUTE, bool::MPV_FORMAT)?;
        }

        Ok(())
    }

    fn time_change(&mut self, data: Property) -> Result<()> {
        log::trace!("Received property-change event [{data}]");

        if let Some(time_pos) = data.data() {
            if let Some(s) = self.worker.get_skip_segment(time_pos) {
                self.skip(s) // Skip segments are priority
            } else if let Some(s) = self.worker.get_mute_segment(time_pos) {
                self.mute(s)
            } else {
                self.reset()
            }
        } else {
            Ok(())
        }
    }

    fn mute_change(&mut self, data: Property) {
        log::trace!("Received property-change event [{data}]");

        if data.data() == Some(false) {
            self.mute_sponsorblock = false;
        };
    }

    fn client_message(&mut self, data: ClientMessage) -> Result<()> {
        log::trace!("Received client-message event");

        match data.args().as_slice() {
            ["key-binding", "poi", "u-", ..] => self.poi_requested()?,
            ["segments-fetched"] => self.segments_fetched(),
            _ => {}
        };
        Ok(())
    }

    fn end_file(&mut self) -> Result<()> {
        log::trace!("Received end-file event");

        self.worker.stop();
        self.unobserve_property(REPL_PROP_TIME)?;
        self.unobserve_property(REPL_PROP_MUTE)?;
        self.reset()?;
        Ok(())
    }

    fn skip(&mut self, working_segment: Segment) -> Result<()> {
        self.set_property(NAME_PROP_TIME, working_segment.segment[1])?;
        osd_info!(self, Duration::from_secs(8), "Skipped segment {working_segment}");
        Ok(())
    }

    fn mute(&mut self, working_segment: Segment) -> Result<()> {
        // Working only if entering a new segment
        if self.mute_segment != Some(working_segment.clone()) {
            // If muted by the plugin do it again just for the log or if not muted do it
            let mute: bool = self.get_property(NAME_PROP_MUTE)?;
            if self.mute_sponsorblock || !mute {
                self.set_property(NAME_PROP_MUTE, true)?;
                self.mute_sponsorblock = true;
                osd_info!(self, Duration::from_secs(8), "Mutting segment {working_segment}");
            } else {
                log::info!("Muttable segment found but mute was requested by user prior segment. Ignoring");
            }

            self.mute_segment = Some(working_segment);
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // Working only if exiting segment
        if self.mute_segment.is_some() {
            // If muted the by plugin then unmute
            if self.mute_sponsorblock {
                self.set_property(NAME_PROP_MUTE, false)?;
                log::info!("Unmutting");
                self.mute_sponsorblock = false;
            } else {
                log::info!("Muttable segment(s) ended but mute value was modified. Ignoring");
            }

            self.mute_segment = None;
        }

        Ok(())
    }

    fn poi_requested(&mut self) -> Result<()> {
        if let Some(time_pos) = self.worker.get_video_poi() {
            self.set_property(NAME_PROP_TIME, time_pos)?;
            osd_info!(self, Duration::from_secs(8), "Jumping to highlight at {time_pos}");
        }
        Ok(())
    }

    fn segments_fetched(&mut self) {
        if let Some(category) = self.worker.get_video_category() {
            let _ = osd!(
                self,
                Duration::from_secs(10),
                "This entire video is labeled as '{category}' and is too tightly integrated to be able to separate"
            );
        }
    }
}

impl Deref for Client {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Self::Target {
        Handle::from_ptr(self.handle)
    }
}

impl DerefMut for Client {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        Handle::from_ptr(self.handle)
    }
}
